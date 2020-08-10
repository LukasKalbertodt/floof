use std::fmt;
use anyhow::Result;
use crate::{
    Context, Task,
    prelude::*,
};

mod copy;
mod command;
mod http;
mod watch;

pub use self::{
    copy::Copy,
    command::Command,
    http::Http,
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
    fn start(&self, task: &Task, ctx: &Context) -> Result<Box<dyn RunningOperation>>;

    /// Starts the operation and immediately runs it to completion.
    fn run(&self, task: &Task, ctx: &Context) -> Result<Outcome> {
        self.start(task, ctx)?.finish()
    }

    /// Validates the operation's configuration. The implementing type can
    /// return an error here to indicate that the configuration has some logic
    /// errors. This is called after parsing the configuration file.
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// An operation that has been started and that is potentially still running.
pub trait RunningOperation {
    /// Blocks and runs the operation to completion.
    fn finish(&mut self) -> Result<Outcome>;

    /// Checks if the operation is already finished and returns its outcome.
    /// Otherwise, returns `None` but does not block!
    fn try_finish(&mut self) -> Result<Option<Outcome>>;

    /// Cancels the operation.
    fn cancel(&mut self) -> Result<()>;
}

/// Result of executing an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Failure,
}

/// An implementation of `RunningOperation` for operations that are very short
/// running and already finish inside `start`.
struct Finished(Outcome);
impl RunningOperation for Finished {
    fn finish(&mut self) -> Result<Outcome> {
        Ok(self.0)
    }
    fn try_finish(&mut self) -> Result<Option<Outcome>> {
        Ok(Some(self.0))
    }
    fn cancel(&mut self) -> Result<()> {
        panic!("bug: called cancel but step is already finished")
    }
}
