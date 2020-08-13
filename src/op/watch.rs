//! Watching directories and trigger operations whenever something changed.
//! Defines the `watch` and `on-change` operations.

use std::{
    time::{Duration, Instant},
    path::Path,
    sync::mpsc::{self, RecvTimeoutError, TryRecvError, Receiver},
    thread::{self, JoinHandle},
};
use notify::{Watcher, RecursiveMode};
use serde::Deserialize;
use crate::{
    Context,
    ui,
    prelude::*,
    context::FrameKind,
};
use super::{Finished, Operation, Operations, Outcome, RunningOperation, ParentKind};


/// We unfortunately can't "listen" on a channel and a child process at the same
/// time, waking up when either changes. So instead, we need to do some busy
/// waiting. Not completely busy, fortunately. This duration specifies the
/// timeout when waiting for the channel.
const BUSY_WAIT_DURATION: Duration = Duration::from_millis(20);

/// The duration for which we debounce watch events.
const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

/// Operation `on-change`. Wraps another operation and only executes it if the
/// operation was triggered by a file change in the parent `watch` operation. If
/// not used as a direct child of `watch`, validation will error.
#[derive(Debug, Clone, Deserialize)]
pub struct OnChange(Box<dyn Operation>);

impl OnChange {
    pub const KEYWORD: &'static str = "on-change";
}

impl Operation for OnChange {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    fn start(&self, ctx: &Context) -> Result<Box<dyn RunningOperation + '_>> {
        if ctx.top_frame.get_var::<TriggeredByChange>().expect("bug: not in watch context").0 {
            self.0.start(ctx)
        } else {
            Ok(Box::new(Finished(Outcome::Success)))
        }
    }

    fn validate(&self, parent: ParentKind<'_>, _config: &Config) -> Result<()> {
        if parent != ParentKind::Operation("watch") {
            bail!("`on-change` operation can only be used in the `run` \
                array of a `watch` operation");
        }

        Ok(())
    }
}

#[derive(Clone, Copy)]
struct TriggeredByChange(bool);

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watch {
    paths: Vec<String>,
    run: Operations,
    debounce: Option<u64>,
    // TODO: flag to enable polling?
}

impl Watch {
    pub const KEYWORD: &'static str = "watch";
}

impl Operation for Watch {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    fn start(&self, ctx: &Context) -> Result<Box<dyn RunningOperation + '_>> {
        // Prepare watcher.
        let (raw_event_tx, raw_event_rx) = mpsc::channel();
        let mut watcher = notify::raw_watcher(raw_event_tx)?;

        let base = ctx.workdir();
        for path in &self.paths {
            let mut path = Path::new(path).to_path_buf();
            if path.is_relative() {
                path = base.join(path);
            }

            if !path.exists() {
                bail!("path '{}' does not exist", path.display());
            }

            watcher.watch(&path, RecursiveMode::Recursive)?;
        }

        // Spawn a thread that listens for raw watcher notifications. We need
        // this thread to acquire a (mostly) exact timestamp from when the event
        // was received. The executor thread might be waiting, so it can't
        // listen for watch events all the time.
        let (event_tx, event_rx) = mpsc::channel();
        thread::spawn(move || {
            for _raw_event in raw_event_rx {
                // TODO: send path
                let event = Event {
                    time: Instant::now(),
                };
                event_tx.send(event).expect("executor thread unexpectedly stopped");
            }

            // Here, the channel has been closed, meaning that the watcher has
            // been dropped. This only happens if the main thread is cancelled,
            // so we just stop.
        });

        let config = self.clone();
        let ctx = ctx.clone();
        let executor = thread::spawn(move || executor(&ctx, config, event_rx));

        Ok(Box::new(Running {
            watcher: Some(watcher),
            executor: Some(executor),
        }))
    }
}

struct Event {
    time: Instant,
}

struct Running<'a> {
    watcher: Option<notify::RecommendedWatcher>,
    executor: Option<JoinHandle<Result<()>>>,
}

impl RunningOperation for Running<'_> {
    fn finish(&mut self, _ctx: &Context) -> Result<Outcome> {
        // This will never return as there is no "finish condition" for this
        // operation.
        self.executor.take().unwrap()
            .join().expect("executor thread panicked")?;
        panic!("executor thread unexpectedly stopped");
    }
    fn try_finish(&mut self, _ctx: &Context) -> Result<Option<Outcome>> {
        Ok(None)
    }
    fn cancel(&mut self) -> Result<()> {
        // By dropping the watcher, the watch thread stop due to disconnected
        // channel, leading the executor to stop because of that too.
        self.watcher.take();
        self.executor.take().unwrap()
            .join().expect("executor thread panicked")
    }
}

/// The code run by the executor thread. It receives file change notifications
/// from the watcher thread (`triggers`), debounces those and executes tasks.
fn executor(
    ctx: &Context,
    config: Watch,
    incoming_events: Receiver<Event>,
) -> Result<()> {
    // If the channel disconnects, that means the watcher thread has stopped
    // which means the main thread tries to stop everything.
    macro_rules! on_disconnect {
        () => { return Ok(()) };
    }

    /// This function is better modelled as state machine instead of using
    /// control flow structures. These are the states this function can be in.
    #[derive(Debug)]
    enum State {
        Initial,
        WaitingForChange,
        Debouncing(Instant),
        RunOnChange,
    }

    let op_ctx = ctx.fork_op("watch");
    let debounce_duration = config.debounce
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_DEBOUNCE_DURATION);
    let pretty_debounce_duration = if debounce_duration >= Duration::from_secs(1) {
        format!("{:.1?}", debounce_duration)
    } else {
        format!("{:.0?}", debounce_duration)
    };

    // Runs all given operations and returns the new state, or `None` if the
    // channel has disconnected.
    let run_operations = |is_on_change: bool| -> Result<Option<State>> {
        op_ctx.top_frame.insert_var(TriggeredByChange(is_on_change));
        for op in &config.run {
            let mut running = op.start(&op_ctx)?;

            // We have a busy loop here: We regularly check if new events
            // arrived, in which case we will cancel the operation and enter the
            // debouncing state. We also regularly check if the operation is
            // done. If so, we will proceed with the next operation.
            loop {
                match incoming_events.recv_timeout(BUSY_WAIT_DURATION) {
                    Ok(event) => {
                        msg!(
                            stop [ctx] ["watch"] "change detected while executing operations! \
                                Cancelling operations, then debouncing for {}...",
                            pretty_debounce_duration,
                        );
                        running.cancel()?;
                        return Ok(Some(State::Debouncing(event.time)))
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if running.try_finish(&op_ctx)?.is_some() {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => return Ok(None),
                }
            };
        }

        Ok(Some(State::WaitingForChange))
    };


    // Run the state machine forever. Only way to exit is `on_disconnect!()`.
    let mut state = State::Initial;
    loop {
        match state {
            State::Initial => {
                msg!(- [ctx]["watch"] "executing operations once on startup...");
                state = match run_operations(false)? {
                    Some(new_state) => new_state,
                    None => on_disconnect!(),
                };
            }
            State::WaitingForChange => {
                match incoming_events.recv() {
                    Err(_) => on_disconnect!(),
                    Ok(event) => {
                        verbose!(
                            waiting [ctx] ["watch"] "change detected, debouncing for {}...",
                            pretty_debounce_duration,
                        );
                        state = State::Debouncing(event.time);
                    }
                }
            }
            State::Debouncing(trigger_time) => {
                // Sleep a bit before checking for new events.
                let since_trigger = trigger_time.elapsed();
                if let Some(duration) = debounce_duration.checked_sub(since_trigger) {
                    thread::sleep(duration);
                }

                match incoming_events.try_recv() {
                    // There has been a new event in the debounce time, so we
                    // continue our debounce wait.
                    Ok(event) => {
                        // We clear the whole queue here to avoid waiting
                        // `debounce_duration` for every event.
                        let mut new_trigger_time = event.time;
                        loop {
                            match incoming_events.try_recv() {
                                Ok(event) => new_trigger_time = event.time,
                                Err(TryRecvError::Disconnected) => on_disconnect!(),
                                Err(TryRecvError::Empty) => break,
                            }
                        }

                        // TODO: trace output

                        state = State::Debouncing(new_trigger_time);
                    }

                    // In this case, nothing new has happened and we can finally
                    // proceed.
                    Err(TryRecvError::Empty) => state = State::RunOnChange,

                    Err(TryRecvError::Disconnected) => on_disconnect!(),
                };
            }
            State::RunOnChange => {
                msg!(fire [ctx]["watch"] "change detected: running all operations...");
                state = match run_operations(true)? {
                    Some(new_state) => new_state,
                    None => on_disconnect!(),
                };
            }
        }
    }
}
