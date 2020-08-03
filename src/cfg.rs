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
// use crate::step;
use crate::{
    Operation, Task,
    op::{Command, Copy},
};


/// The default filename from which to load the configuration.
pub const DEFAULT_FILENAME: &str = "watchboi.yaml";

// pub const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(500);


/// The root configuration object.
#[derive(Debug, Deserialize)]
#[serde(from = "HashMap<String, TaskConfig>")]
pub struct Config {
    pub tasks: HashMap<String, Task>,
}

impl From<HashMap<String, TaskConfig>> for Config {
    fn from(tasks: HashMap<String, TaskConfig>) -> Self {
        let tasks = tasks.into_iter()
            .map(|(name, TaskConfig(operations))| (name.clone(), Task { name, operations }))
            .collect();

        Self { tasks }
    }
}

/// A task is simply defined by a list of operations.
#[derive(Debug, Deserialize)]
#[serde(from = "Vec<Box<dyn Operation>>")]
struct TaskConfig(Vec<Box<dyn Operation>>);

impl From<Vec<Box<dyn Operation>>> for TaskConfig {
    fn from(ops: Vec<Box<dyn Operation>>) -> Self {
        Self(ops)
    }
}



macro_rules! define_ops {
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

define_ops![
    "command" => Command,
    "copy" => Copy,
];



// #[derive(Debug, Clone, Deserialize)]
// #[serde(deny_unknown_fields)]
// pub struct Http {
//     pub addr: Option<SocketAddr>,
//     pub proxy: Option<SocketAddr>,
//     pub ws_addr: Option<SocketAddr>,
// }

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

    // pub fn has_reload_step(&self) -> bool {
    //     self.tasks.values().any(|a| a.has_reload_step())
    // }
}

// impl Task {
//     pub fn run_steps(&self) -> &[Step] {
//         Self::steps(&self.run)
//     }
//     pub fn on_start_steps(&self) -> &[Step] {
//         Self::steps(&self.on_start)
//     }
//     pub fn on_change_steps(&self) -> &[Step] {
//         Self::steps(&self.on_change)
//     }

//     fn steps(steps: &Option<Vec<Step>>) -> &[Step] {
//         match steps {
//             None => &[],
//             Some(v) => v,
//         }
//     }

//     fn has_reload_step(&self) -> bool {
//         self.run_steps().iter()
//                 .chain(self.on_start_steps())
//                 .chain(self.on_change_steps())
//                 .any(|s| matches!(s, Step::Reload(_)))
//     }

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

// impl Http {
//     pub fn addr(&self) -> SocketAddr {
//         self.addr.unwrap_or(([127, 0, 0, 1], 8030).into())
//     }

//     pub fn ws_addr(&self) -> SocketAddr {
//         self.ws_addr.unwrap_or(([127, 0, 0, 1], 8031).into())
//     }

//     fn validate(&self) -> Result<()> {
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
