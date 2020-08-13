use std::fmt;
use anyhow::Result;
use crate::{
    Context, Config,
    prelude::*,
};

mod workdir;
mod copy;
mod command;
mod http;
mod run_task;
mod watch;

pub use self::{
    workdir::{WorkDir, SetWorkDir},
    copy::Copy,
    command::Command,
    http::Http,
    run_task::RunTask,
    watch::Watch,
};


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
    fn start(&self, ctx: &Context) -> Result<Box<dyn RunningOperation + '_>>;

    /// Starts the operation and immediately runs it to completion.
    fn run(&self, ctx: &Context) -> Result<Outcome> {
        self.start(ctx)?.finish(ctx)
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

/// An operation that has been started and that is potentially still running.
pub trait RunningOperation {
    /// Blocks and runs the operation to completion.
    fn finish(&mut self, ctx: &Context) -> Result<Outcome>;

    /// Checks if the operation is already finished and returns its outcome.
    /// Otherwise, returns `None` but does not block!
    fn try_finish(&mut self, ctx: &Context) -> Result<Option<Outcome>>;

    /// Cancels the operation.
    fn cancel(&mut self) -> Result<()>;
}

/// Result of executing an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Outcome {
    Success,
    Failure,
}

impl Outcome {
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure)
    }

    pub fn to_exit_code(&self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
        }
    }
}

/// An implementation of `RunningOperation` for operations that are very short
/// running and already finish inside `start`.
struct Finished(Outcome);
impl RunningOperation for Finished {
    fn finish(&mut self, _ctx: &Context) -> Result<Outcome> {
        Ok(self.0)
    }
    fn try_finish(&mut self, _ctx: &Context) -> Result<Option<Outcome>> {
        Ok(Some(self.0))
    }
    fn cancel(&mut self) -> Result<()> {
        panic!("bug: called cancel but step is already finished")
    }
}

pub enum ParentKind<'a> {
    /// Operation of a task with the given name.
    Task(&'a str),
    /// Suboperation of another operation with the given keyword.
    Operation(&'a str),
}
