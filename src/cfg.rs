//! Configuration, usually loaded from `watchboi.yaml`.

use std::{
    collections::HashMap,
    fmt,
    fs,
    net::SocketAddr,
    path::Path,
    time::Duration,
};
use anyhow::{bail, Context, Result};
use serde::{Deserializer, Deserialize, de::{self, MapAccess, SeqAccess, Visitor}};
use crate::step;


/// The default filename from which to load the configuration.
pub const DEFAULT_FILENAME: &str = "watchboi.yaml";

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
    pub run: Option<Vec<Step>>,
    pub on_start: Option<Vec<Step>>,
    pub on_change: Option<Vec<Step>>,
}

/// One of different kinds of steps that watchboi can execute.
#[derive(Debug, Clone)]
pub enum Step {
    Command(step::Command),
    Copy(step::Copy),
    Reload(step::Reload),

    // When adding new variants, also adjust the `match_tag` invocation inside
    // the `Deserialize` impl!
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StepVisitor;
        impl<'de> Visitor<'de> for StepVisitor {
            type Value = Step;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string, an array or a map with a single field")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Step::Command(step::Command::simple(v)))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut v = Vec::new();

                while let Some(value) = seq.next_element()? {
                    v.push(value);
                }

                Ok(Step::Command(step::Command::explicit(v)))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let tag = map.next_key::<String>()?
                    .ok_or_else(|| de::Error::invalid_length(0, &"1"))?;

                /// Macro to avoid duplication of valid tags
                macro_rules! match_tag {
                    ($($tag:literal => $variant:ident,)+ ) => {
                        match &*tag {
                            $(
                                $tag => Ok(Step::$variant(map.next_value()?)),
                            )+
                            other => Err(de::Error::unknown_variant(other, &[$($tag),+])),
                        }

                    };
                }

                match_tag! {
                    "command" => Command,
                    "copy" => Copy,
                    "reload" => Reload,
                }
            }
        }

        // The use of `deserialize_any` is discouraged as this makes using
        // non-selfdescribing formats (usually, many binary formats) impossible.
        // But we know that we will use YAML and we don't really have a choice
        // here as we indeed can be deserialized from different types.
        deserializer.deserialize_any(StepVisitor)
    }
}


#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Http {
    pub addr: Option<SocketAddr>,
    pub proxy: Option<SocketAddr>,
    pub ws_addr: Option<SocketAddr>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watcher {
    pub debounce: Option<u32>,
}


impl Config {
    /// Loads and validates the configuration from the specified path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read(path)
            .context(format!("failed to read contents of '{}'", path.display()))?;
        let config: Self = serde_yaml::from_slice(&content)
            .context("failed to deserialize YAML file")?;
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

            if action.has_reload_step() && self.http.is_none() {
                bail!(
                    "action '{}' includes a 'reload' step, but no HTTP server is configured \
                        (top level key 'http' is missing)",
                    name,
                );
            }
        }

        Ok(())
    }

    pub fn has_reload_step(&self) -> bool {
        self.actions.values().any(|a| a.has_reload_step())
    }
}

impl Action {
    pub fn run_steps(&self) -> &[Step] {
        Self::steps(&self.run)
    }
    pub fn on_start_steps(&self) -> &[Step] {
        Self::steps(&self.on_start)
    }
    pub fn on_change_steps(&self) -> &[Step] {
        Self::steps(&self.on_change)
    }

    fn steps(steps: &Option<Vec<Step>>) -> &[Step] {
        match steps {
            None => &[],
            Some(v) => v,
        }
    }

    fn has_reload_step(&self) -> bool {
        self.run_steps().iter()
                .chain(self.on_start_steps())
                .chain(self.on_change_steps())
                .any(|s| matches!(s, Step::Reload(_)))
    }

    fn validate(&self) -> Result<()> {
        if self.on_change.is_some() && self.watch.is_none() {
            bail!("field 'on_change' requires 'watch' to be specified \
                (otherwise it would never run)");
        }

        if self.watch.is_some() && (self.on_change.is_none() && self.run.is_none()) {
            bail!("field 'watch' is specified, but neither 'run' nor 'on_change' commands \
                are specified, which makes no sense");
        }

        for step in self.on_start_steps() {
            step.validate().context("invalid 'on_start' steps")?;
        }
        for step in self.on_change_steps() {
            step.validate().context("invalid 'on_change' steps")?;
        }
        for step in self.run_steps() {
            step.validate().context("invalid 'run' steps")?;
        }

        Ok(())
    }
}

impl Http {
    pub fn addr(&self) -> SocketAddr {
        self.addr.unwrap_or(([127, 0, 0, 1], 8030).into())
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

impl Step {
    fn validate(&self) -> Result<()> {
        match self {
            Step::Command(v) => v.validate(),
            Step::Copy(v) => v.validate(),
            Step::Reload(v) => v.validate(),
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
