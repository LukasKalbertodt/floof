//! Definitions of all possible kinds of steps that can be executed by watchboi.

use std::fmt;
use anyhow::{bail, Result};
use serde::Deserialize;


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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reload;

impl Reload {
    pub fn validate(&self) -> Result<()> {
        Ok(())
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

impl fmt::Display for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Simple(s) => s.fmt(f),
            Self::Explicit(v) => {
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
