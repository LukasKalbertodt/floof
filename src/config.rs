//! Configuration, usually loaded from `watchboi.toml`.

use std::{
    fmt,
    fs,
    path::Path,
};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;


/// The default filename from which to load the configuration.
pub const DEFAULT_FILENAME: &str = "watchboi.toml";

/// The root configuration object.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub actions: Option<HashMap<String, Action>>,
    pub proxy: Option<Proxy>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Action {
    pub base: Option<String>,
    pub on_start: Option<Vec<Command>>,
    pub watch: Option<Vec<String>>,
    pub on_change: Option<Vec<Command>>
}

#[derive(Debug, Clone, Deserialize)]
pub struct Proxy {
    pub inject_js: bool,
}

/// A command specification (a application name/path and its arguments).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Command {
    /// A single string which will be split at whitespace boundaries. Fine for
    /// most commands.
    Simple(String),

    /// An array of strings that is passed to `std::process::Command` like this.
    /// Required for when arguments in the command contain whitespace.
    Explicit(Vec<String>),
}

impl Config {
    /// Loads and validates the configuration from the specified path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read(path)
            .context(format!("failed to read contents of '{}'", path.display()))?;
        let config: Self = toml::from_slice(&content)
            .context("failed to parse config file as TOML")?;
        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if let Some(proxy) = &self.proxy {
            proxy.validate()?;
        }

        for (key, action) in self.actions.iter().flatten() {
            action.validate().context(format!("invalid configuration for action '{}'", key))?;
        }

        Ok(())
    }
}

impl Action {
    fn validate(&self) -> Result<()> {
        if self.on_change.is_some() && self.watch.is_none() {
            bail!("field 'on_change' requires 'watch' to be specified \
                (otherwise it would never run)");
        }

        for command in self.on_start.iter().flatten() {
            command.validate().context("failed validation of 'on_start' commands")?;
        }
        for command in self.on_change.iter().flatten() {
            command.validate().context("failed validation of 'on_change' commands")?;
        }

        Ok(())
    }
}

impl Proxy {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl Command {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Simple(s) => {
                if s.trim().is_empty() {
                    bail!("empty command is invalid");
                }
            }
            Self::Explicit(v) => {
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

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Command::Simple(s) => s.fmt(f),
            Command::Explicit(v) => {
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

// watch settings:
// - debounce length
// - polling?
// - these settings per action?
//
// per action:
// - base which paths are relative to and commands are executed in
