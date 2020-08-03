use serde::Deserialize;
use std::convert::{TryFrom, TryInto};
use super::Operation;

#[derive(Debug, Clone, Deserialize)]
pub struct Command {
    run: ProgramAndArgs,

    /// What working directory to execute the command in.
    workdir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawProgramAndArgs")]
struct ProgramAndArgs {
    /// The command to run.
    program: String,

    /// Arguments for the command.
    args: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawProgramAndArgs {
    Simple(String),
    Explicit(Vec<String>),
}

impl TryFrom<RawProgramAndArgs> for ProgramAndArgs {
    type Error = String;

    fn try_from(src: RawProgramAndArgs) -> Result<Self, Self::Error> {
        match src {
            RawProgramAndArgs::Simple(s) => {
                if s.is_empty() || s.chars().all(|c| c.is_whitespace()) {
                    return Err("command string is empty".into());
                }

                let mut split = s.split_whitespace();
                let program = split.next().unwrap().to_owned(); // checked above
                let args: Vec<_> = split.map(|s| s.to_owned()).collect();

                Ok(Self { program, args })
            }
            RawProgramAndArgs::Explicit(v) => {
                if v.is_empty() {
                    return Err("empty list as command specification".into());
                }

                if v.iter().any(|f| f.is_empty() || f.chars().all(|c| c.is_whitespace())) {
                    return Err("empty fragment in command specification".into());
                }

                let program = v[0].clone();
                let args = v.into_iter().skip(1).collect();

                Ok(Self { program, args })
            }
        }
    }
}

impl From<ProgramAndArgs> for Command {
    fn from(src: ProgramAndArgs) -> Self {
        Self {
            run: src,
            workdir: None,
        }
    }
}


impl Command {
    pub fn from_simple(s: &str) -> Result<Self, String> {
        Ok(ProgramAndArgs::try_from(RawProgramAndArgs::Simple(s.into()))?.into())
    }

    pub fn from_explicit(v: Vec<String>) -> Result<Self, String> {
        Ok(ProgramAndArgs::try_from(RawProgramAndArgs::Explicit(v))?.into())
    }
}

impl Operation for Command {
}
