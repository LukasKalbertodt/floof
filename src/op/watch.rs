//! Watching directories and trigger operations whenever something changed.
//! Defines the `watch` and `on-change` operations.

use std::{
    time::Duration,
    path::Path,
};
use crossbeam_channel::Receiver;
use notify::{Event, Watcher, RecursiveMode, RecommendedWatcher};
use serde::Deserialize;
use crate::prelude::*;
use super::{
    Operation, Operations, Outcome, RunningOperation, ParentKind,
    OP_NO_OUTCOME_ERROR, BUG_CANCEL_DISCONNECTED,
};


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

    fn start(&self, ctx: &Context) -> Result<RunningOperation> {
        if ctx.top_frame.get_var::<TriggeredByChange>().expect("bug: not in watch context").0 {
            self.0.start(ctx)
        } else {
            Ok(RunningOperation::finished(Outcome::Success))
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

    fn start(&self, ctx: &Context) -> Result<RunningOperation> {
        // Prepare watcher.
        //
        // We pipe all incoming events into a channel. The receiver of the
        // channel and the watcher itself will be sent to the thread execution
        // this operation.
        let (raw_event_tx, raw_event_rx) = crossbeam_channel::unbounded();
        let mut watcher: RecommendedWatcher = Watcher::new_immediate(move |event| {
            raw_event_tx.send(event).expect("bug: executor thread unexpectedly ended");
        })?;

        // Add paths to watch.
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

        let config = self.clone();
        let running = RunningOperation::new(ctx, |ctx, cancel_request| {
            // We need to move the watcher in this thread to keep it alive for
            // the whole duration of this operation. If it's dropped, the files are not being watched anymore.
            let _watcher = watcher;

            run(ctx, config, raw_event_rx, cancel_request)
        });

        Ok(running)
    }
}

fn run(
    ctx: &Context,
    config: Watch,
    fs_events: Receiver<Result<Event, notify::Error>>,
    cancel_request: Receiver<()>,
) -> Result<Outcome> {
    const BUG_WATCHER_GONE: &str = "bug: watcher unexpectedly stopped and dropped channel";

    /// This function is better modelled as state machine instead of using
    /// control flow structures. These are the states this function can be in.
    #[derive(Debug)]
    enum State {
        WaitingForChange,
        Debouncing,
        Run { triggered_by_change: bool },
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

    // Runs all given operations and returns the new state, or `None` if this
    // operation got cancelled.
    let run_operations = |is_on_change: bool| -> Result<Option<State>> {
        op_ctx.top_frame.insert_var(TriggeredByChange(is_on_change));
        for op in &config.run {
            let mut running = op.start(&op_ctx)?;

            crossbeam_channel::select! {
                recv(running.outcome()) -> outcome => {
                    let outcome = outcome.expect(OP_NO_OUTCOME_ERROR)?;
                    if !outcome.is_success() {
                        verbose!(
                            - [ctx] - "'{}' operation failed → stopping (no further operations of \
                                this task are ran)",
                            op.keyword(),
                        );
                        break;
                    }
                },
                recv(cancel_request) -> result => {
                    result.expect(BUG_CANCEL_DISCONNECTED);
                    running.cancel()?;
                    return Ok(None);
                },
                recv(fs_events) -> event => {
                    event.expect(BUG_WATCHER_GONE)?;

                    msg!(
                        stop [ctx] ["watch"] "change detected while executing operations! \
                            Cancelling operations, then debouncing for {}...",
                        pretty_debounce_duration,
                    );
                    running.cancel()?;
                    return Ok(Some(State::Debouncing));
                },
            }
        }

        Ok(Some(State::WaitingForChange))
    };


    // Run the state machine forever. Only way to exit is `on_disconnect!()`.
    let mut state = State::Run { triggered_by_change: false };
    loop {
        match state {
            State::Run { triggered_by_change } => {
                if triggered_by_change {
                    msg!(fire [ctx]["watch"] "change detected: running all operations...");
                } else {
                    msg!(- [ctx]["watch"] "executing operations once on startup...");
                }

                state = match run_operations(triggered_by_change)? {
                    Some(new_state) => new_state,
                    None => return Ok(Outcome::Cancelled),
                };
            }
            State::WaitingForChange => {
                crossbeam_channel::select! {
                    recv(cancel_request) -> result => {
                        result.expect(BUG_CANCEL_DISCONNECTED);
                        return Ok(Outcome::Cancelled);
                    },
                    recv(fs_events) -> event => {
                        event.expect(BUG_WATCHER_GONE)?;

                        verbose!(
                            waiting [ctx] ["watch"] "change detected, debouncing for {}...",
                            pretty_debounce_duration,
                        );
                        state = State::Debouncing;
                    },
                }
            }
            State::Debouncing => {
                // Sleep a bit before checking for new events.
                crossbeam_channel::select! {
                    recv(cancel_request) -> result => {
                        result.expect(BUG_CANCEL_DISCONNECTED);
                        return Ok(Outcome::Cancelled);
                    },
                    recv(fs_events) -> event => {
                        event.expect(BUG_WATCHER_GONE)?;
                        state = State::Debouncing;
                    },
                    default(debounce_duration) => {
                        state = State::Run { triggered_by_change: true };
                    },
                }
            }
        }
    }
}
