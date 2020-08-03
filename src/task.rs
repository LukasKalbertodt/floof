// use std::{
//     sync::mpsc::{channel, Sender, Receiver, TryRecvError, RecvTimeoutError},
//     thread, path::Path, time::{Duration, Instant},
// };

use anyhow::{bail, Context as _, Result};
// use notify::{Watcher, RecursiveMode};

// use crate::{
//     cfg,
//     context::Context,
//     step::{Outcome, Step as _},
// };

use crate::Operation;


#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub operations: Vec<Box<dyn Operation>>,
}

impl Task {
    pub fn validate(&self) -> Result<()> {
        for op in &self.operations {
            op.validate()?;
        }

        Ok(())
    }
}


// /// Run all `on_start` tasks of the given task and, if the task watches files,
// /// start threads which watch those files and trigger corresponding `on_change`
// /// steps.
// pub fn run(name: &str, task: &cfg::Task, ctx: &Context) -> Result<()> {
//     // Run all commands that we are supposed to run on start.
//     let mut on_start_tasks = task.on_start_steps().to_vec();
//     if task.watch.is_none() {
//         // If this task is not watching anything, we need to execute the tasks
//         // here once. Otherwise, they are executed in the executor thread.
//         on_start_tasks.extend(task.run_steps().iter().cloned());
//     }

//     for step in on_start_tasks {
//         let outcome = step.execute(name, task, ctx)?;
//         if outcome == Outcome::Failure {
//             bail!("'on_start' step for task '{}' failed", name);
//         }
//     }


//     // If `watch` is specified, we start two threads and start watching files.
//     if let Some(watched_paths) = &task.watch {
//         let (trigger_tx, trigger_rx) = channel();
//         let (watch_init_tx, watch_init_rx) = channel();

//         // Spawn watcher thread.
//         {
//             let name = name.to_owned();
//             let task = task.clone();
//             let watched_paths = watched_paths.clone();
//             ctx.spawn_thread(move |ctx| {
//                 watch(name, task, &watched_paths, trigger_tx, watch_init_tx, ctx)
//             });
//             let _ = watch_init_rx.recv();
//         }

//         // Spawn executor thread.
//         {
//             let name = name.to_owned();
//             let task = task.clone();
//             ctx.spawn_thread(move |ctx| executor(name, task, trigger_rx, ctx));
//         }
//     }

//     Ok(())
// }

// /// Executed by the watcher thread: creates a watcher and sends incoming events
// /// to the executor thread. Does no debouncing.
// fn watch(
//     name: String,
//     task: cfg::Task,
//     watched_paths: &[String],
//     triggers: Sender<Instant>,
//     init_done: Sender<()>,
//     ctx: &Context,
// ) -> Result<()> {
//     let (tx, rx) = channel();
//     let mut watcher = notify::raw_watcher(tx).unwrap();

//     for path in watched_paths {
//         let path = match &task.base {
//             Some(base) => Path::new(base).join(path),
//             None => Path::new(path).into(),
//         };

//         if !path.exists() {
//             bail!("path '{}' of task '{}' does not exist", path.display(), name);
//         }

//         watcher.watch(&path, RecursiveMode::Recursive)?;
//     }

//     ctx.ui.watching(&name, &watched_paths);
//     init_done.send(()).unwrap();

//     // Send one trigger for each raw watch event.
//     for _ in rx {
//         triggers.send(Instant::now()).expect("executor thread unexpectedly stopped");
//     }

//     // The loop above should loop forever.
//     bail!("watcher unexpectedly stopped");
// }

// /// We unfortunately can't "listen" on a channel and a child process at the same
// /// time, waking up when either changes. So instead, we need to do some busy
// /// waiting. Not completely busy, fortunately. This duration specifies the
// /// timeout when waiting for the channel.
// const BUSY_WAIT_DURATION: Duration = Duration::from_millis(20);

// /// The code run by the executor thread. It receives file change notifications
// /// from the watcher thread (`triggers`), debounces and executes tasks.
// fn executor(
//     name: String,
//     task: cfg::Task,
//     triggers: Receiver<Instant>,
//     ctx: &Context,
// ) -> Result<()> {
//     /// Just a handler for `RecvError::Disconnected` which panics. It should
//     /// never happen that the watch thread stops but the executor thread does
//     /// not.
//     fn on_disconnect() -> ! {
//         panic!("watcher thread unexpectedly stopped");
//     };

//     /// This function is better modelled as state machine instead of using
//     /// control flow structures. These are the states this function can be in.
//     #[derive(Debug)]
//     enum State {
//         Initial,
//         WaitingForChange,
//         Debouncing(Instant),
//         RunOnChange,
//     }

//     let debounce_duration = ctx.config.watcher.as_ref()
//         .map(|c| c.debounce())
//         .unwrap_or(cfg::DEFAULT_DEBOUNCE_DURATION);

//     // Runs all given steps and returns the new state.
//     let run_steps = |trigger, steps: &[cfg::Step]| -> Result<State> {
//         for step in steps {
//             let mut running = step.start(&name, &task, ctx)?;

//             // We have a busy loop here: We regularly check if new triggers
//             // arrived, in which case we will cancel the step and enter the
//             // debouncing state. We also regularly check if the step is done. If
//             // so, we will proceed with the next step.
//             loop {
//                 match triggers.recv_timeout(BUSY_WAIT_DURATION) {
//                     Ok(t) => {
//                         ctx.ui.change_detected(&name, debounce_duration);
//                         running.cancel()?;
//                         return Ok(State::Debouncing(t))
//                     }
//                     Err(RecvTimeoutError::Timeout) => {
//                         if running.try_finish()?.is_some() {
//                             break;
//                         }
//                     }
//                     Err(RecvTimeoutError::Disconnected) => on_disconnect(),
//                 }
//             };
//         }

//         Ok(State::WaitingForChange)
//     };


//     let on_start_tasks = task.run_steps();
//     let on_change_tasks = task.run_steps()
//         .iter()
//         .chain(task.on_change_steps())
//         .cloned()
//         .collect::<Vec<_>>();

//     let mut state = State::Initial;

//     loop {
//         match state {
//             State::Initial => {
//                 state = run_steps("on_start", &on_start_tasks)?;
//             }
//             State::WaitingForChange => {
//                 let trigger_time = triggers.recv().unwrap_or_else(|_| on_disconnect());
//                 ctx.ui.change_detected(&name, debounce_duration);
//                 state = State::Debouncing(trigger_time);
//             }
//             State::Debouncing(trigger_time) => {
//                 // Sleep a bit before checking for new events.
//                 let since_trigger = trigger_time.elapsed();
//                 if let Some(duration) = debounce_duration.checked_sub(since_trigger) {
//                     thread::sleep(duration);
//                 }

//                 match triggers.try_recv() {
//                     // There has been a new event in the debounce time, so we
//                     // continue our debounce wait.
//                     Ok(mut new_trigger_time) => {
//                         // We clear the whole queue here to avoid waiting
//                         // `debounce_duration` for every event.
//                         loop {
//                             match triggers.try_recv() {
//                                 Ok(t) => new_trigger_time = t,
//                                 Err(TryRecvError::Disconnected) => on_disconnect(),
//                                 Err(TryRecvError::Empty) => break,
//                             }
//                         }

//                         state = State::Debouncing(new_trigger_time);
//                     }

//                     // In this case, nothing new has happened and we can finally
//                     // proceed.
//                     Err(TryRecvError::Empty) => state = State::RunOnChange,

//                     Err(TryRecvError::Disconnected) => on_disconnect(),
//                 };
//             }
//             State::RunOnChange => {
//                 ctx.ui.run_on_change_handlers(&name);
//                 state = run_steps("on_change", &on_change_tasks)?;
//             }
//         }
//     }
// }
