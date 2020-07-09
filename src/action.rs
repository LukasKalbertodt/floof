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


/// Run all `on_start` tasks of the given action and, if the action watches
/// files, start threads which watch those files and trigger corresponding
/// `on_change` actions.
pub fn run(name: &str, action: &config::Action, ctx: &Context) -> Result<()> {
    // Run all commands that we are supposed to run on start.
    let mut on_start_tasks = action.on_start.clone().unwrap_or_default();
    if action.watch.is_none() {
        // If this action is not watching anything, we need to execute the tasks
        // here once. Otherwise, they are executed in the executor thread.
        on_start_tasks.extend(action.run.clone().unwrap_or_default());
    }

    for command in on_start_tasks {
        ctx.ui.run_command("on_start", &command);
        let status = command.to_std(&action.base).status()
            .context(format!("failed to run `{}`", command))?;

        if !status.success() {
            bail!("'on_start' command for action '{}' failed (`{}`)", name, command);
        }
    }


    // If `watch` is specified, we start two threads and start watching files.
    if let Some(watched_paths) = &action.watch {
        let (trigger_tx, trigger_rx) = channel();
        let (watch_init_tx, watch_init_rx) = channel();

        // Spawn watcher thread.
        {
            let name = name.to_owned();
            let action = action.clone();
            let watched_paths = watched_paths.clone();
            ctx.spawn_thread(move |ctx| {
                watch(name, action, &watched_paths, trigger_tx, watch_init_tx, ctx)
            });
            let _ = watch_init_rx.recv();
        }

        // Spawn executor thread.
        {
            let name = name.to_owned();
            let action = action.clone();
            ctx.spawn_thread(move |ctx| executor(name, action, trigger_rx, ctx));
        }
    }

    Ok(())
}

/// Executed by the watcher thread: creates a watcher and sends incoming events
/// to the executor thread. Does no debouncing.
fn watch(
    name: String,
    action: config::Action,
    watched_paths: &[String],
    triggers: Sender<Instant>,
    init_done: Sender<()>,
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
    init_done.send(()).unwrap();

    // Send one trigger for each raw watch event.
    for _ in rx {
        triggers.send(Instant::now()).expect("executor thread unexpectedly stopped");
    }

    // The loop above should loop forever.
    bail!("watcher unexpectedly stopped");
}

/// We unfortunately can't "listen" on a channel and a child process at the same
/// time, waking up when either changes. So instead, we need to do some busy
/// waiting. Not completely busy, fortunately. This duration specifies the
/// timeout when waiting for the channel.
const BUSY_WAIT_DURATION: Duration = Duration::from_millis(20);

/// The code run by the executor thread. It receives file change notifications
/// from the watcher thread (`triggers`), debounces and executes tasks.
fn executor(
    name: String,
    action: config::Action,
    triggers: Receiver<Instant>,
    ctx: &Context,
) -> Result<()> {
    /// Just a handler for `RecvError::Disconnected` which panics. It should
    /// never happen that the watch thread stops but the executor thread does
    /// not.
    fn on_disconnect() -> ! {
        panic!("watcher thread unexpectedly stopped");
    };

    /// This function is better modelled as state machine instead of using
    /// control flow structures. These are the states this function can be in.
    #[derive(Debug)]
    enum State {
        Initial,
        WaitingForChange,
        Debouncing(Instant),
        RunOnChange,
    }

    let debounce_duration = ctx.config.watcher.as_ref()
        .map(|c| c.debounce())
        .unwrap_or(config::DEFAULT_DEBOUNCE_DURATION);

    // Runs all given tasks and returns the new state.
    let run_tasks = |trigger, tasks: &[config::Command]| -> Result<State> {
        for command in tasks {
            ctx.ui.run_command(trigger, command);
            let mut child = command.to_std(&action.base).spawn()?;

            // We have a busy loop here: We regularly check if new triggers
            // arrived, in which case we will kill the command and restart the
            // outer loop. We also regularly check if the child is done. If so,
            // we will exit this inner loop and proceed with the next command.
            loop {
                match triggers.recv_timeout(BUSY_WAIT_DURATION) {
                    Ok(t) => {
                        ctx.ui.change_detected(&name, debounce_duration);
                        child.kill()?;
                        return Ok(State::Debouncing(t))
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if child.try_wait()?.is_some() {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => on_disconnect(),
                }
            };
        }

        Ok(State::WaitingForChange)
    };


    let on_start_tasks = action.run.clone().unwrap_or_default();
    let on_change_tasks = action.run.clone()
        .into_iter()
        .chain(action.on_change.clone())
        .flatten()
        .collect::<Vec<_>>();

    let mut state = State::Initial;

    loop {
        match state {
            State::Initial => {
                state = run_tasks("on_start", &on_start_tasks)?;
            }
            State::WaitingForChange => {
                let trigger_time = triggers.recv().unwrap_or_else(|_| on_disconnect());
                ctx.ui.change_detected(&name, debounce_duration);
                state = State::Debouncing(trigger_time);
            }
            State::Debouncing(trigger_time) => {
                // Sleep a bit before checking for new events.
                let since_trigger = trigger_time.elapsed();
                if let Some(duration) = debounce_duration.checked_sub(since_trigger) {
                    thread::sleep(duration);
                }

                match triggers.try_recv() {
                    // There has been a new event in the debounce time, so we
                    // continue our debounce wait.
                    Ok(mut new_trigger_time) => {
                        // We clear the whole queue here to avoid waiting
                        // `debounce_duration` for every event.
                        loop {
                            match triggers.try_recv() {
                                Ok(t) => new_trigger_time = t,
                                Err(TryRecvError::Disconnected) => on_disconnect(),
                                Err(TryRecvError::Empty) => break,
                            }
                        }

                        state = State::Debouncing(new_trigger_time);
                    }

                    // In this case, nothing new has happened and we can finally
                    // proceed.
                    Err(TryRecvError::Empty) => state = State::RunOnChange,

                    Err(TryRecvError::Disconnected) => on_disconnect(),
                };
            }
            State::RunOnChange => {
                ctx.ui.run_on_change_handlers(&name);
                if action.reload == Some(config::Reload::Early) {
                    ctx.request_reload(name.clone());
                }

                state = run_tasks("on_change", &on_change_tasks)?;

                if action.reload == Some(config::Reload::Late) {
                    ctx.request_reload(name.clone());
                }
            }
        }
    }
}


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
