//! Configuration, usually loaded from `watchboi.toml`.

use std::{
    fmt,
    fs,
    net::SocketAddr,
    path::Path,
};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{time::Duration, collections::HashMap};


/// The default filename from which to load the configuration.
pub const DEFAULT_FILENAME: &str = "watchboi.toml";

pub const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

/// The root configuration object.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub actions: HashMap<String, Action>,
    pub http: Option<Http>,
    pub watcher: Option<Watcher>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub base: Option<String>,
    pub watch: Option<Vec<String>>,
    pub run: Option<Vec<Command>>,
    pub on_start: Option<Vec<Command>>,
    pub on_change: Option<Vec<Command>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Http {
    pub addr: Option<SocketAddr>,
    pub proxy: Option<SocketAddr>,
    pub auto_reload: Option<bool>,
    pub ws_addr: Option<SocketAddr>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watcher {
    pub debounce: Option<u32>,
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
        config.validate()
            .context("invalid config file: logic errors were found")?;

        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if let Some(http) = &self.http {
            http.validate()?;
        }
        if let Some(watcher) = &self.watcher {
            watcher.validate()?;
        }

        for (name, action) in &self.actions {
            action.validate().context(format!("invalid configuration for action '{}'", name))?;
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

        if self.watch.is_some() && (self.on_change.is_none() && self.run.is_none()) {
            bail!("field 'watch' is specified, but neither 'run' nor 'on_change' commands \
                are specified, which makes no sense");
        }

        for command in self.on_start.iter().flatten() {
            command.validate().context("invalid 'on_start' commands")?;
        }
        for command in self.on_change.iter().flatten() {
            command.validate().context("invalid 'on_change' commands")?;
        }
        for command in self.run.iter().flatten() {
            command.validate().context("invalid 'run' commands")?;
        }

        Ok(())
    }
}

impl Http {
    pub fn addr(&self) -> SocketAddr {
        self.addr.unwrap_or(([127, 0, 0, 1], 8030).into())
    }

    pub fn auto_reload(&self) -> bool {
        self.auto_reload.unwrap_or(true)
    }

    pub fn ws_addr(&self) -> SocketAddr {
        self.ws_addr.unwrap_or(([127, 0, 0, 1], 8031).into())
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl Watcher {
    fn validate(&self) -> Result<()> {
        Ok(())
    }

    pub fn debounce(&self) -> Duration {
        self.debounce
            .map(|ms| Duration::from_millis(ms as u64))
            .unwrap_or(DEFAULT_DEBOUNCE_DURATION)
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
