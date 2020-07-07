use std::{
    process::Command,
    sync::mpsc::{channel, Sender, Receiver, TryRecvError, RecvTimeoutError},
    thread, path::Path, time::{Duration, Instant},
};

use anyhow::{bail, Context as _, Result};
use notify::{Watcher, RecursiveMode};

use crate::{
    config,
    context::Context,
};


pub fn run(name: &str, action: &config::Action, ctx: &Context) -> Result<()> {
    // Run all commands that we are supposed to run on start.
    if let Some(on_start_commands) = &action.on_start {
        for command in on_start_commands {
            ctx.ui.run_command("on_start", command);
            let status = command.to_std(&action.base).status()
                .context(format!("failed to run `{}`", command))?;

            if !status.success() {
                bail!("'on_start' command for action '{}' failed (`{}`)", name, command);
            }
        }
    }

    // If `watch` is specified, we start a thread and start watching files.
    if let Some(watched_paths) = &action.watch {
        let (trigger_tx, trigger_rx) = channel();

        // Spawn watcher thread.
        {
            let name = name.to_owned();
            let action = action.clone();
            let watched_paths = watched_paths.clone();
            ctx.spawn_thread(move |ctx| watch(name, action, &watched_paths, trigger_tx, ctx));
        }

        // Spawn executor thread.
        {
            let name = name.to_owned();
            let action = action.clone();
            ctx.spawn_thread(move |ctx| executor(name, action, trigger_rx, ctx));
        }
    } else {
        // TODO: run `run` commands once
    }

    Ok(())
}

fn watch(
    name: String,
    action: config::Action,
    watched_paths: &[String],
    triggers: Sender<Instant>,
    ctx: &Context,
) -> Result<()> {
    let (tx, rx) = channel();
    let mut watcher = notify::raw_watcher(tx).unwrap();

    for path in watched_paths {
        let path = match &action.base {
            Some(base) => Path::new(base).join(path),
            None => Path::new(path).into(),
        };

        if !path.exists() {
            bail!("path '{}' of action '{}' does not exist", path.display(), name);
        }

        watcher.watch(&path, RecursiveMode::Recursive)?;
    }

    ctx.ui.watching(&name, &watched_paths);

    // We send one initial trigger to already run all `run` tasks.
    triggers.send(Instant::now()).expect("executor thread unexpectedly stopped");

    // Send one trigger for each raw watch event.
    for _ in rx {
        triggers.send(Instant::now()).expect("executor thread unexpectedly stopped");
    }

    // The loop above should loop forever.
    bail!("watcher unexpectedly stopped");
}

fn executor(
    name: String,
    action: config::Action,
    triggers: Receiver<Instant>,
    ctx: &Context,
) -> Result<()> {

    let debounce_duration = ctx.config.watcher.as_ref()
        .and_then(|c| c.debounce)
        .map(|ms| Duration::from_millis(ms as u64))
        .unwrap_or(config::DEFAULT_DEBOUNCE_DURATION);


    let on_disconnect = || -> ! {
        panic!("watcher thread unexpectedly stopped");
    };

    let run_tasks = action.run.unwrap_or_default();
    let all_tasks = run_tasks.clone().into_iter()
        .chain(action.on_change.unwrap_or_default())
        .collect::<Vec<_>>();

    for (i, mut trigger_time) in triggers.iter().enumerate() {
        let is_artificial = i == 0;

        if !is_artificial {
            ctx.ui.change_detected(&name);
        }

        'debounce: loop {
            macro_rules! restart {
                ($trigger_time:expr) => {{
                    trigger_time = $trigger_time;
                    continue 'debounce;
                }};
            }

            let since_trigger = trigger_time.elapsed();
            if let Some(duration) = debounce_duration.checked_sub(since_trigger) {
                thread::sleep(duration);
            }

            match triggers.try_recv() {
                Ok(t) => restart!(t),
                Err(TryRecvError::Disconnected) => on_disconnect(),

                // In this case, nothing new has happened and we can finally
                // proceed.
                Err(TryRecvError::Empty) => {},
            };

            // Start executing the commands
            let tasks = if is_artificial {
                &run_tasks
            } else {
                ctx.ui.run_on_change_handlers(&name);
                &all_tasks
            };

            // TODO: only send signal of autorefresh is on
            let _ = ctx.request_reload();
            for command in tasks {
                ctx.ui.run_command("on_change", command);
                let mut child = command.to_std(&action.base).spawn()?;

                // We have a busy loop here: We regularly check if new triggers
                // arrived, in which case we will kill the command and restart the
                // outer loop. We also regularly check if the child is done. If so,
                // we will exit this inner loop and proceed with the next command.
                loop {
                    match triggers.recv_timeout(BUSY_WAIT_DURATION) {
                        Ok(t) => {
                            child.kill()?;
                            restart!(t);
                        }
                        Err(RecvTimeoutError::Disconnected) => on_disconnect(),
                        Err(RecvTimeoutError::Timeout) => {
                            if child.try_wait()?.is_some() {
                                break;
                            }
                        }
                    }
                };
            }

            // All commands finished, we can exit the debounce loop.
            break 'debounce;
        }
    }

    on_disconnect();
}

const BUSY_WAIT_DURATION: Duration = Duration::from_millis(20);

impl config::Command {
    /// Creates a `std::process::Command` from the command specified in the
    /// configuration.
    fn to_std(&self, working_dir: &Option<String>) -> Command {
        let (program, args) = match self {
            config::Command::Simple(s) => {
                let mut split = s.split_whitespace();
                let program = split.next()
                    .expect("bug: validation should ensure string is not empty");
                let args: Vec<_> = split.collect();

                (program, args)
            }
            config::Command::Explicit(v) => {
                let program = v.get(0).expect("bug: validation should ensure vector is not empty");
                let args = v[1..].iter().map(|s| s.as_str()).collect();

                (program.as_str(), args)
            }
        };

        let mut command = Command::new(&program);
        command.args(args);
        if let Some(working_dir) = working_dir {
            command.current_dir(working_dir);
        }
        command
    }
}
