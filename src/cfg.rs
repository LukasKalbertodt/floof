//! Configuration, usually loaded from `watchboi.yaml`.

use std::{
    collections::HashMap,
    fmt,
    fs,
    net::SocketAddr,
    path::Path,
    time::Duration, convert::TryFrom,
};
use anyhow::{bail, Context, Result};
use serde::{Deserializer, Deserialize, de::{self, MapAccess, SeqAccess, Visitor}};
use crate::{
    Operation, Task,
    op::{Command, Copy, Http},
};


/// The default filename from which to load the configuration.
pub const DEFAULT_FILENAME: &str = "watchboi.yaml";


/// The root configuration object.
#[derive(Debug, Deserialize)]
#[serde(from = "HashMap<String, Operations>")]
pub struct Config {
    pub tasks: HashMap<String, Task>,
}

impl From<HashMap<String, Operations>> for Config {
    fn from(tasks: HashMap<String, Operations>) -> Self {
        let tasks = tasks.into_iter()
            .map(|(name, operations)| (name.clone(), Task { name, operations }))
            .collect();

        Self { tasks }
    }
}

/// A task is defined by a list of operations.
type Operations = Vec<Box<dyn Operation>>;


// Helper macro to avoid code duplication. Implements `Deserialize` for
// `Box<dyn Operation>`.
macro_rules! impl_deserialize_for_op {
    ($($tag:literal => $ty:ident ,)*) => {
        impl<'de> Deserialize<'de> for Box<dyn Operation> {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct OpVisitor;
                impl<'de> Visitor<'de> for OpVisitor {
                    type Value = Box<dyn Operation>;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("a string, an array or a map with a single field")
                    }

                    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        let command = Command::from_simple(v).map_err(de::Error::custom)?;
                        Ok(Box::new(command))
                    }

                    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        let mut v = Vec::new();
                        while let Some(value) = seq.next_element()? {
                            v.push(value);
                        }

                        let command = Command::from_explicit(v).map_err(de::Error::custom)?;
                        Ok(Box::new(command))
                    }

                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        let tag = map.next_key::<String>()?
                            .ok_or_else(|| de::Error::invalid_length(0, &"1"))?;

                        match tag.as_str() {
                            $(
                                $tag => {
                                    let op: $ty = map.next_value()?;
                                    Ok(Box::new(op))
                                }
                            )*
                            other => Err(de::Error::unknown_variant(other, &[$($tag),+])),
                        }
                    }
                }

                // The use of `deserialize_any` is discouraged as this makes using
                // non-selfdescribing formats (usually, many binary formats) impossible.
                // But we know that we will use YAML and we don't really have a choice
                // here as we indeed can be deserialized from different types.
                deserializer.deserialize_any(OpVisitor)
            }
        }
    };
}

impl_deserialize_for_op![
    "command" => Command,
    "copy" => Copy,
    "http" => Http,
];


// #[derive(Debug, Clone, Deserialize)]
// #[serde(deny_unknown_fields)]
// pub struct Watcher {
//     pub debounce: Option<u32>,
// }

        // impl TryFrom<Vec<RawOperation>> for TaskConfig {
        //     type Error = String;

        //     fn try_from(ops: Vec<RawOperation>) -> Result<Self, Self::Error> {
        //         let ops = ops.into_iter().map(|op| {
        //             let op: Box<dyn Operation> = match op {
        //                 RawOperation::Str(s) => Box::new(Command::from_simple(s)?),
        //                 RawOperation::Arr(v) => Box::new(Command::from_explicit(v)?),
        //                 RawOperation::Tagged(TaggedOperation::Command(x)) => Box::new(x),
        //                 RawOperation::Tagged(TaggedOperation::Copy(x)) => Box::new(x),
        //             };

        //             Ok(op)
        //         }).collect::<Result<_, String>>()?;

        //         Ok(Self(ops))
        //     }
        // }

impl Config {
    /// Loads and validates the configuration from the specified path.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let default_path = Path::new(DEFAULT_FILENAME);
        match path {
            Some(path) => {
                Config::load_from(path)
                    .context(format!("failed to load configuration from '{}'", path.display()))
            }
            None if default_path.exists() && default_path.is_file() => {
                Config::load_from(default_path).with_context(|| {
                    format!(
                        "failed to load configuration from default location '{}' \
                            (file exists, but is invalid)",
                        DEFAULT_FILENAME,
                    )
                })
            }
            None => {
                eprintln!("No configuration found!");
                eprintln!("A `watchboi.toml` has to exist in the current directory or \
                    the path to the configuration file has to be given via the \
                    `--config`/`-c` argument");
                bail!("no configuration found");
            }
        }
    }

    fn load_from(path: impl AsRef<Path>) -> Result<Self> {
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
        for task in self.tasks.values() {
            task.validate().context(format!("invalid configuration for task '{}'", task.name))?;
        }

        Ok(())
    }
}

// impl Task {
//     fn validate(&self) -> Result<()> {
//         if self.on_change.is_some() && self.watch.is_none() {
//             bail!("field 'on_change' requires 'watch' to be specified \
//                 (otherwise it would never run)");
//         }

//         if self.watch.is_some() && (self.on_change.is_none() && self.run.is_none()) {
//             bail!("field 'watch' is specified, but neither 'run' nor 'on_change' commands \
//                 are specified, which makes no sense");
//         }

//         for step in self.on_start_steps() {
//             step.validate().context("invalid 'on_start' steps")?;
//         }
//         for step in self.on_change_steps() {
//             step.validate().context("invalid 'on_change' steps")?;
//         }
//         for step in self.run_steps() {
//             step.validate().context("invalid 'run' steps")?;
//         }

//         Ok(())
//     }
// }

// impl Watcher {
//     fn validate(&self) -> Result<()> {
//         Ok(())
//     }

//     pub fn debounce(&self) -> Duration {
//         self.debounce
//             .map(|ms| Duration::from_millis(ms as u64))
//             .unwrap_or(DEFAULT_DEBOUNCE_DURATION)
//     }
// }

// impl Step {
//     fn validate(&self) -> Result<()> {
//         match self {
//             Step::Command(v) => v.validate(),
//             Step::Copy(v) => v.validate(),
//             Step::Reload(v) => v.validate(),
//         }
//     }
// }

// watch settings:
// - debounce length
// - polling?
// - these settings per task?
