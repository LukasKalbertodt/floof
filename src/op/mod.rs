use std::{thread, fmt};
use anyhow::Result;
use crossbeam_channel::{Sender, Receiver};
use crate::prelude::*;

mod workdir;
mod concurrently;
mod copy;
mod command;
mod http;
mod run_task;
mod watch;

pub use self::{
    workdir::{WorkDir, SetWorkDir},
    concurrently::Concurrently,
    copy::Copy,
    command::Command,
    http::{Http, Reload},
    run_task::RunTask,
    watch::{OnChange, Watch},
};


const OP_NO_OUTCOME_ERROR: &str = "bug: operation did not send outcome";
const BUG_CANCEL_DISCONNECTED: &str = "bug: cancel channel disconnected";

/// Multiple dynamically dispatched operations.
pub type Operations = Vec<Box<dyn Operation>>;

/// An abstract operation. Is part of a task and can be part of other
/// operations.
pub trait Operation: fmt::Debug + 'static + Send + Sync {
    /// Returns the keyword that is used in the configuration to refer to this
    /// operation. This is a method instead of a constant to keep this trait
    /// object safe.
    fn keyword(&self) -> &'static str;

    /// Starts the operation.
    fn start(&self, ctx: &Context) -> Result<RunningOperation>;

    /// Starts the operation and immediately runs it to completion.
    fn run(&self, ctx: &Context) -> Result<Outcome> {
        self.start(ctx)?.outcome().recv().expect(OP_NO_OUTCOME_ERROR)
    }

    /// Validates the operation's configuration. The implementing type can
    /// return an error here to indicate that the configuration has some logic
    /// errors. This is called after parsing the configuration file.
    fn validate(&self, _parent: ParentKind<'_>, _config: &Config) -> Result<()> {
        Ok(())
    }

    fn dyn_clone(&self) -> Box<dyn Operation>;
}

impl Clone for Box<dyn Operation> {
    fn clone(&self) -> Self {
        self.dyn_clone()
    }
}

/// Result of executing an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Outcome {
    Success,
    Failure,
    Cancelled,
}

impl Outcome {
    pub fn is_success(&self) -> bool {
        *self == Self::Success
    }

    pub fn to_exit_code(&self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::Cancelled => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentKind<'a> {
    /// Operation of a task with the given name.
    Task(&'a str),
    /// Suboperation of another operation with the given keyword.
    Operation(&'a str),
}

pub struct RunningOperation {
    cancel: Option<Sender<()>>,
    outcome: Receiver<Result<Outcome>>,
}

impl RunningOperation {
    pub fn start<F>(ctx: &Context, op: F) -> Self
    where
        F: 'static + Send + Sync + FnOnce(&Context, Receiver<()>) -> Result<Outcome>,
    {
        let (cancel_tx, cancel_rx) = crossbeam_channel::bounded(0);
        let (outcome_tx, outcome_rx) = crossbeam_channel::bounded(1);
        let ctx = ctx.clone();

        thread::spawn(move || {
            let result = op(&ctx, cancel_rx);

            // We ignore a disconnected channel, as we will stop anyway.
            let _ = outcome_tx.send(result);
        });

        Self {
            cancel: Some(cancel_tx),
            outcome: outcome_rx,
        }
    }

    pub fn finished(outcome: Outcome) -> Self {
        let (outcome_tx, outcome_rx) = crossbeam_channel::bounded(1);
        outcome_tx.send(Ok(outcome)).unwrap();

        Self {
            cancel: None,
            outcome: outcome_rx,
        }
    }

    /// Returns the receiver end of a channel that will receive the `outcome` of
    /// the operation once it's finished.
    ///
    /// The operation sends the outcome into the channel exactly once at its
    /// end.
    ///
    /// This method always returns the same receiver when called on the same
    /// `self`.
    pub fn outcome(&self) -> &Receiver<Result<Outcome>> {
        &self.outcome
    }

    /// Cancels the operation. If the operation was already finished, this does
    /// nothing and returns `Ok(())`.
    pub fn cancel(&mut self) -> Result<()> {
        // If this is `None`, then there is no corresponding receiver and the
        // outcome is already in `self.outcome`.
        if let Some(cancel) = &self.cancel {
            // We ignore both results: if the other thread has already ended,
            // that's fine by us; it is supposed to do that anyway.
            let _ = cancel.send(());
            let _ = self.outcome.recv();
        }

        Ok(())
    }
}
