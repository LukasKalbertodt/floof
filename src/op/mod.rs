use std::fmt;
use anyhow::Result;
use crate::prelude::*;

mod command;
mod concurrently;
mod copy;
mod http;
mod run_task;
mod watch;
mod workdir;

pub use self::{
    command::Command,
    concurrently::Concurrently,
    copy::Copy,
    http::{Http, Reload},
    run_task::RunTask,
    watch::{OnChange, Watch},
    workdir::{WorkDir, SetWorkDir},
};



/// Multiple dynamically dispatched operations.
pub type Operations = Vec<Box<dyn Operation>>;

/// An abstract operation. Is part of a task and can be part of other
/// operations.
#[async_trait::async_trait]
pub trait Operation: fmt::Debug + 'static + Send + Sync {
    /// Returns the keyword that is used in the configuration to refer to this
    /// operation. This is a method instead of a constant to keep this trait
    /// object safe.
    fn keyword(&self) -> &'static str;

    /// Runs the operation.
    async fn run(&self, ctx: &Context) -> Result<Outcome>;

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
