use std::{sync::mpsc::channel, path::Path};
use anyhow::{bail, Context, Result};
use structopt::StructOpt;
use crate::{
    args::Args,
    config::Config,
};

mod action;
mod args;
mod config;



fn main() -> Result<()> {
    let args = Args::from_args();

    let config = match &args.config {
        Some(path) => {
            Config::load(path)
                .context(format!("failed to load configuration from '{}'", path.display()))?
        }
        None => {
            let path = Path::new(config::DEFAULT_FILENAME);
            if path.exists() && path.is_file() {
                Config::load(path).with_context(|| {
                    format!(
                        "failed to load configuration from default location '{}' \
                            (file exists, but is invalid)",
                        config::DEFAULT_FILENAME,
                    )
                })?
            } else {
                bail!("no configuration!");
            }
        }
    };

    // Run each action (actions with `on_change` commands will spawn a thread).
    let (errors_tx, errors_rx) = channel();
    for (name, action) in config.actions.into_iter().flatten() {
        action::run(name, action, &errors_tx)?;
    }
    drop(errors_tx);

    match errors_rx.recv() {
        // There are no thread running, so we can just quit.
        Err(_) => println!("----- No action has 'on_change' commands, we're done here"),
        // A thread returned an error.
        Ok(e) => return Err(e),
    }

    Ok(())
}
