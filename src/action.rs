use std::{
    process::Command,
    sync::mpsc::{channel, Sender, Receiver, TryRecvError, RecvTimeoutError},
    thread, path::Path, time::{Duration, Instant},
};

use anyhow::{bail, Context, Error, Result};
use notify::{Watcher, RecursiveMode};

use crate::config;


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

pub fn run(name: String, action: config::Action, errors: &Sender<Error>) -> Result<()> {
    // Run all commands that we are supposed to run on start.
    if let Some(on_start_commands) = &action.on_start {
        println!("===== Running 'on_start' commands for action '{}'", name);

        for command in on_start_commands {
            println!("----- Running: {}", command);
            let status = command.to_std(&action.base).status()
                .context(format!("failed to run `{}`", command))?;

            if !status.success() {
                bail!("'on_start' command for action '{}' failed (`{}`)", name, command);
            }
        }
    }

    if action.on_change.is_some() {
        let (trigger_tx, trigger_rx) = channel();

        let watch_errors = errors.clone();
        let watched_paths = action.watch.expect("action.watch is None");
        thread::spawn(move || {
            if let Err(e) = watch(name, &watched_paths, trigger_tx) {
                let _ = watch_errors.send(e);
            }
        });

        let executor_errors = errors.clone();
        let on_change = action.on_change.expect("action.on_change is None");
        thread::spawn(move || {
            if let Err(e) = executor(on_change, trigger_rx) {
                let _ = executor_errors.send(e);
            }
        });
    }

    Ok(())
}

fn watch(name: String, watched_paths: &[String], triggers: Sender<Instant>) -> Result<()> {
    let (tx, rx) = channel();
    let mut watcher = notify::raw_watcher(tx).unwrap();

    for path in watched_paths {
        let path = Path::new(path);
        if !path.exists() {
            bail!("path '{}' of action '{}' does not exist", path.display(), name);
        }

        watcher.watch(path, RecursiveMode::Recursive)?;
    }

    for _ in rx {
        let now = Instant::now();
        triggers.send(now).expect("executor thread unexpectedly stopped");
    }

    // The loop above should loop forever.
    bail!("watcher unexpectedly stopped");
}

fn executor(on_change: Vec<config::Command>, triggers: Receiver<Instant>) -> Result<()> {
    let on_disconnect = || -> ! {
        panic!("watcher thread unexpectedly stopped");
    };


    for mut trigger_time in triggers.iter() {
        'debounce: loop {
            macro_rules! restart {
                ($trigger_time:expr) => {{
                    trigger_time = $trigger_time;
                    continue 'debounce;
                }};
            }

            let since_trigger = trigger_time.elapsed();
            if let Some(duration) = DEBOUNCE_DURATION.checked_sub(since_trigger) {
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
            for command in &on_change {
                println!("----- Running: {}", command);
                let mut child = command.to_std().spawn()?;

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

const DEBOUNCE_DURATION: Duration = Duration::from_millis(500);
const BUSY_WAIT_DURATION: Duration =  Duration::from_millis(20);
