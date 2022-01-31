//! Watching directories and trigger operations whenever something changed.
//! Defines the `watch` and `on-change` operations.

use std::{
    time::Duration,
    path::Path,
};
use notify::{Watcher, RecursiveMode, RecommendedWatcher};
use serde::Deserialize;
use tokio::sync::watch;

use crate::prelude::*;
use super::{Operation, Operations, Outcome, ParentKind};


/// The duration for which we debounce watch events.
const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(250);

/// Operation `on-change`. Wraps another operation and only executes it if the
/// operation was triggered by a file change in the parent `watch` operation. If
/// not used as a direct child of `watch`, validation will error.
#[derive(Debug, Clone, Deserialize)]
pub struct OnChange(Box<dyn Operation>);

impl OnChange {
    pub const KEYWORD: &'static str = "on-change";
}

#[async_trait::async_trait]
impl Operation for OnChange {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    async fn run(&self, ctx: &Context) -> Result<Outcome> {
        // TODO: validate this when parsing AND ... only top frame? Probably
        // just "closest var" I think.
        if ctx.top_frame.get_var::<TriggeredByChange>().expect("bug: not in watch context").0 {
            self.0.run(ctx).await
        } else {
            Ok(Outcome::Success)
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

#[async_trait::async_trait]
impl Operation for Watch {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    async fn run(&self, ctx: &Context) -> Result<Outcome> {
        // ===== Prepare watcher =================================================================
        //
        // The watcher will run in its own thread. The closure given to the
        // watcher is not async, so the easiest way to get events into our async
        // world is to send them through a channel. Once the `watcher` is
        // dropped, it no longer watches anything.
        let (watch_event_tx, mut watch_events) = watch::channel(());
        let mut watcher = RecommendedWatcher::new(move |_ev| {
            watch_event_tx.send(()).expect("bug: executor thread unexpectedly ended");
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


        // ===== Listen for events and run operations ===========================================
        let op_ctx = ctx.fork_op("watch");
        let debounce_duration = self.debounce
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_DEBOUNCE_DURATION);
        let pretty_debounce_duration = if debounce_duration >= Duration::from_secs(1) {
            format!("{:.1?}", debounce_duration)
        } else {
            format!("{:.0?}", debounce_duration)
        };


        // Run the state machine forever.
        let mut state = State::Run { triggered_by_change: false };
        'main: loop {
            match state {
                State::WaitingForChange => {
                    watch_events.changed().await.expect(BUG_WATCHER_GONE);
                    verbose!(
                        waiting [ctx] ["watch"] "change detected, debouncing for {}...",
                        pretty_debounce_duration,
                    );

                    state = State::Debouncing;
                }

                State::Debouncing => {
                    match tokio::time::timeout(debounce_duration, watch_events.changed()).await {
                        // A new FS event arrived before the debounce could
                        // finish. We just make sure the watcher hasn't been
                        // dropped and remain in the `Debouncing` state.
                        Ok(new_event) => new_event.expect(BUG_WATCHER_GONE),

                        // This means the timeout (debounce period) elapsed.
                        Err(_) => state = State::Run { triggered_by_change: true },
                    }
                }

                State::Run { triggered_by_change } => {
                    if triggered_by_change {
                        msg!(fire [ctx]["watch"] "change detected: running all operations...");
                    } else {
                        verbose!(- [ctx]["watch"] "executing operations once on startup...");
                    }

                    op_ctx.top_frame.insert_var(TriggeredByChange(triggered_by_change));
                    for op in &self.run {
                        let running = op.run(&op_ctx);

                        tokio::select! {
                            outcome = running => {
                                if !outcome?.is_success() {
                                    verbose!(
                                        - [ctx] - "'{}' operation failed → stopping (no further \
                                            operations of this task will run)",
                                        op.keyword(),
                                    );

                                    break;
                                }
                            }
                            res = watch_events.changed() => {
                                res.expect(BUG_WATCHER_GONE);
                                msg!(
                                    stop [ctx] ["watch"] "change detected while executing \
                                        operations! Cancelling operations, then debouncing \
                                        for {}...",
                                    pretty_debounce_duration,
                                );

                                state = State::Debouncing;
                                continue 'main;
                            }
                        }
                    }

                    state = State::WaitingForChange;
                }
            }
        }
    }
}


/// The main loop is better modelled as state machine instead of using control
/// flow structures. These are the possible states.
#[derive(Debug)]
enum State {
    WaitingForChange,
    Debouncing,
    Run { triggered_by_change: bool },
}

const BUG_WATCHER_GONE: &str = "bug: watcher unexpectedly stopped and dropped channel";
