use std::process::Command;
use anyhow::{bail, Context, Result};
use crate::config;

impl config::Command {
    /// Creates a `std::process::Command` from the command specified in the
    /// configuration.
    fn to_std(&self) -> Command {
        let (program, args) = match self {
            config::Command::Simple(s) => {
                let mut split = s.split_whitespace();
                let program = split.next()
                    .expect("bug: validation should ensure string is not empty");
                let args: Vec<_> = split.collect();

                (program, args)
            }
            config::Command::Explicit(v) => {
                let program = v.get(0).expect("bug: validation should ensure vector is not empty");
                let args = v[1..].iter().map(|s| s.as_str()).collect();

                (program.as_str(), args)
            }
        };

        let mut command = Command::new(&program);
        command.args(args);
        command
    }
}

pub fn run(name: &str, action: &config::Action) -> Result<()> {
    // Run all commands that we are supposed to run on start.
    if let Some(on_start_commands) = &action.on_start {
        println!("===== Running 'on_start' commands for action '{}'", name);

        for command in on_start_commands {
            println!("----- Running: {}", command);
            let status = command.to_std().status()
                .context(format!("failed to run `{}`", command))?;

            if !status.success() {
                bail!("'on_start' command for action '{}' failed (`{}`)", name, command);
            }
        }
    }

    Ok(())
}
