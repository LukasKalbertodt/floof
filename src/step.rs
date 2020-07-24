//! Definitions of all possible kinds of steps that can be executed by watchboi.

use std::fmt;
use anyhow::{bail, Context as _, Result};
use serde::Deserialize;
use crate::{cfg, context::Context};


/// Result of executing a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Failure,
}

/// A step that has been started and that is potentially already finished.
pub trait RunningStep {
    /// Blocks and runs the step to completion.
    fn finish(&mut self) -> Result<Outcome>;

    /// Checks if the step is already finished and returns its outcome.
    /// Otherwise, returns `None` but does not block!
    fn try_finish(&mut self) -> Result<Option<Outcome>>;

    /// Cancels the step.
    fn cancel(&mut self) -> Result<()>;
}

/// A defintion of some piece of work that can be executed.
pub trait Step {
    /// Starts the step and returns a handle representing the running step.
    fn start(
        &self,
        action_name: &str,
        action: &cfg::Action,
        ctx: &Context,
    ) -> Result<Box<dyn RunningStep>>;

    /// Starts the step and immediately runs it to completion.
    fn execute(
        &self,
        action_name: &str,
        action: &cfg::Action,
        ctx: &Context,
    ) -> Result<Outcome> {
        self.start(action_name, action, ctx)?.finish()
    }
}

/// An implementation of `RunningStep` for steps that immediately finish.
struct FinishedStep(Outcome);
impl RunningStep for FinishedStep {
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

impl Step for cfg::Step {
    fn start(
        &self,
        action_name: &str,
        action: &cfg::Action,
        ctx: &Context,
    ) -> Result<Box<dyn RunningStep>> {
        match self {
            Self::Command(x) => x.start(action_name, action, ctx),
            Self::Copy(x) => x.start(action_name, action, ctx),
            Self::Reload(x) => x.start(action_name, action, ctx),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Copy {
    src: String,
    dst: String,
}

impl Copy {
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl Step for Copy {
    fn start(
        &self,
        _action_name: &str,
        _action: &cfg::Action,
        _ctx: &Context,
    ) -> Result<Box<dyn RunningStep>> {
        todo!()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reload;

impl Reload {
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl Step for Reload {
    fn start(
        &self,
        action_name: &str,
        _action: &cfg::Action,
        ctx: &Context,
    ) -> Result<Box<dyn RunningStep>> {
        ctx.request_reload(action_name.clone());
        Ok(Box::new(FinishedStep(Outcome::Success)))
    }
}

/// An external command (application name/path and its arguments).
#[derive(Debug, Clone, Deserialize)]
pub struct Command {
    /// The command to run.
    run: CommandKind,
    /// What working directory to execute the command in.
    workdir: Option<String>,
}

/// Commands can either be specified as simple string (in which case the
/// arguments are derived from a simple `split_whitespace`) or as a sequence.
/// The latter is required if any arguments or the program contain whitespace.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CommandKind {
    /// A single string which will be split at whitespace boundaries. Fine for
    /// most commands.
    Simple(String),

    /// An array of strings that is passed to `std::process::Command` like this.
    /// Required for when arguments in the command contain whitespace.
    Explicit(Vec<String>),
}

impl Command {
    /// Shorthand to create a command from a single string with unspecified
    /// working directory.
    pub fn simple(s: impl Into<String>) -> Self {
        Self {
            run: CommandKind::Simple(s.into()),
            workdir: None,
        }
    }

    /// Shorthand to create a command from the explicit representation with
    /// unspecified working directory.
    pub fn explicit(v: Vec<String>) -> Self {
        Self {
            run: CommandKind::Explicit(v),
            workdir: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        match &self.run {
            CommandKind::Simple(s) => {
                if s.trim().is_empty() {
                    bail!("empty command is invalid");
                }
            }
            CommandKind::Explicit(v) => {
                if v.is_empty() {
                    bail!("empty command is invalid");
                }
                if v.iter().any(|s| s.trim().is_empty()) {
                    bail!("segment of command is empty (all segments must be non-empty)");
                }
            }
        }

        Ok(())
    }
}

impl Step for Command {
    fn start(
        &self,
        _action_name: &str,
        action: &cfg::Action,
        ctx: &Context,
    ) -> Result<Box<dyn RunningStep>> {
        ctx.ui.run_command("on_start", self);

        let (program, args) = match &self.run {
            CommandKind::Simple(s) => {
                let mut split = s.split_whitespace();
                let program = split.next()
                    .expect("bug: validation should ensure string is not empty");
                let args: Vec<_> = split.collect();

                (program, args)
            }
            CommandKind::Explicit(v) => {
                let program = v.get(0).expect("bug: validation should ensure vector is not empty");
                let args = v[1..].iter().map(|s| s.as_str()).collect();

                (program.as_str(), args)
            }
        };

        let mut command = std::process::Command::new(&program);
        command.args(args);
        if let Some(working_dir) = self.workdir.as_ref().or(action.base.as_ref()) {
            command.current_dir(working_dir);
        }

        // Run the command and get its status code
        let child = command.spawn().context(format!("failed to spawn `{}`", self))?;
        Ok(Box::new(RunningCommand { child }))
    }
}

struct RunningCommand {
    child: std::process::Child,
}

fn exit_status_to_outcome(status: std::process::ExitStatus) -> Outcome {
    if status.success() {
        Outcome::Success
    } else {
        Outcome::Failure
    }
}

impl RunningStep for RunningCommand {
    fn finish(&mut self) -> Result<Outcome> {
        let status = self.child.wait().context("failed to wait for running process")?;
        Ok(exit_status_to_outcome(status))
    }
    fn try_finish(&mut self) -> Result<Option<Outcome>> {
        let status = self.child.try_wait().context("failed to wait for running process")?;
        Ok(status.map(exit_status_to_outcome))
    }
    fn cancel(&mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.run {
            CommandKind::Simple(s) => s.fmt(f),
            CommandKind::Explicit(v) => {
                let mut first = true;
                for part in v {
                    if first {
                        first = false;
                    } else {
                        write!(f, " ")?;
                    };

                    if part.contains(char::is_whitespace) {
                        write!(f, r#""{}""#, part)?;
                    } else {
                        write!(f, "{}", part)?;
                    }
                }

                Ok(())
            }
        }
    }
}
