use std::{
    fmt,
    convert::TryFrom,
};
use serde::Deserialize;
use crate::{
    Context,
    prelude::*,
};
use super::{Operation, Outcome};

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

impl fmt::Display for ProgramAndArgs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let print = |f: &mut fmt::Formatter, s: &str| {
            if s.contains(char::is_whitespace) {
                write!(f, r#""{}""#, s)
            } else {
                write!(f, "{}", s)
            }
        };


        print(f, &self.program)?;
        for arg in &self.args {
            write!(f, " ")?;
            print(f, arg)?;
        }

        Ok(())
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
    pub const KEYWORD: &'static str = "command";

    pub fn from_simple(s: &str) -> Result<Self, String> {
        Ok(ProgramAndArgs::try_from(RawProgramAndArgs::Simple(s.into()))?.into())
    }

    pub fn from_explicit(v: Vec<String>) -> Result<Self, String> {
        Ok(ProgramAndArgs::try_from(RawProgramAndArgs::Explicit(v))?.into())
    }
}

#[async_trait::async_trait]
impl Operation for Command {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    async fn run(&self, ctx: &Context) -> Result<Outcome> {
        msg!(run [ctx]["command"] "running: {[green]}", self.run);

        let mut command = tokio::process::Command::new(&self.run.program);
        command.kill_on_drop(true);
        command.args(&self.run.args);
        command.current_dir(match &self.workdir {
            Some(workdir) => ctx.join_workdir(&workdir),
            None => ctx.workdir(),
        });

        // Start the command and return a descriptive error if that failed.
        let mut child = command.spawn().map_err(|e| {
            let mut context = format!("failed to spawn `{}`", self.run);
            if e.kind() == std::io::ErrorKind::NotFound {
                context += &format!(
                    " (you probably don't have the command '{}' installed)",
                    self.run.program,
                );
            }

            anyhow::Error::from(e).context(context)
        })?;

        // Check if the process has finished
        let status = child.wait().await.context("error running process")?;
        if status.success() {
            Ok(Outcome::Success)
        } else {
            msg!(warn [ctx]["command"]
                "{[green]} returned non-zero exit code",
                self.run,
            );
            Ok(Outcome::Failure)
        }
    }
}
