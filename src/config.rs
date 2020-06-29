//! Configuration, usually loaded from `watchboi.toml`.

use std::{
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
    actions: Option<HashMap<String, Action>>,
    proxy: Option<Proxy>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Action {
    on_start: Option<Vec<Command>>,
    watch: Option<Vec<String>>,
    on_change: Option<Vec<Command>>
}

#[derive(Debug, Clone, Deserialize)]
pub struct Proxy {
    inject_js: bool,
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

        Ok(())
    }
}

impl Proxy {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

// watch settings:
// - debounce length
// - polling?
// - these settings per action?
//
// per action:
// - base which paths are relative to and commands are executed in
