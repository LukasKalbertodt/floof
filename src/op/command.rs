use std::{
    fmt,
    convert::TryFrom, thread, time::Duration, cmp::min,
};
use serde::Deserialize;
use crate::{
    Context,
    prelude::*,
};
use super::{Operation, Outcome, RunningOperation};

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

impl Operation for Command {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    fn start(&self, ctx: &Context) -> Result<RunningOperation> {
        msg!(run [ctx]["command"] "running: {[green]}", self.run);

        // Build `std::process::Command`.
        let mut command = std::process::Command::new(&self.run.program);
        command.args(&self.run.args);

        let workdir = match &self.workdir {
            Some(workdir) => ctx.join_workdir(&workdir),
            None => ctx.workdir(),
        };
        command.current_dir(workdir);

        // Start the command and return a descriptive error if that failed.
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                let mut context = format!("failed to spawn `{}`", self.run);
                if e.kind() == std::io::ErrorKind::NotFound {
                    context += &format!(
                        " (you probably don't have the command '{}' installed)",
                        self.run.program,
                    );
                }
                return Err(e).context(context);
            }
        };

        // Start a new thread where we regularly check if the command is
        // finished and reply to cancel requests.

        let run_command = self.run.clone();
        let running = RunningOperation::new(ctx, move |ctx, cancel_request| {
            // Soo... unfortunately, `process::Child` has a fairly minimal API.
            // What we want is to wait for the process to finish, but retain the
            // possibility to kill it at any time from another thread. There is
            // no nice way to do that with `process::Child`.
            //
            // The best we can do is busy waiting, checking if the process has
            // finished or whether the process should be killed. To not
            // needlessly burn system resources but also not introduce too much
            // of a delay, we start with a fairly short wait duration which is
            // doubled each iteration until we reach the max duration to wait
            // for. That means that we can still burn through very short
            // commands fairly quickly, while only occasionally checking on long
            // running processes.
            const START_SLEEP_DURATION: Duration = Duration::from_micros(100);
            const MAX_SLEEP_DURATION: Duration = Duration::from_millis(20);

            let mut sleep_duration = START_SLEEP_DURATION;
            loop {
                thread::sleep(sleep_duration);

                // Check if the process has finished
                if let Some(status) = child.try_wait()? {
                    let outcome = if status.success() {
                        Outcome::Success
                    } else {
                        msg!(warn [ctx]["command"]
                            "{[green]} returned non-zero exit code",
                            run_command,
                        );
                        Outcome::Failure
                    };

                    return Ok(outcome);
                }

                // Check if this process should be killed.
                match cancel_request.try_recv() {
                    Ok(_) => {
                        child.kill()?;
                        return Ok(Outcome::Cancelled);
                    }
                    Err(e) if e.is_empty() => {},
                    Err(e) => return Err(e)?,
                }

                // Increase sleep duration.
                sleep_duration = min(sleep_duration * 2, MAX_SLEEP_DURATION);
            }
        });

        Ok(running)
    }
}
