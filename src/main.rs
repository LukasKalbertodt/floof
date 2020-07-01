use std::{sync::mpsc::channel, path::Path};
use anyhow::{bail, Context, Result};
use structopt::StructOpt;
use crate::{
    args::Args,
    config::Config,
    ui::Ui,
};

mod action;
mod args;
mod config;
mod http;
mod ui;


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

    // We collect errors on the main thread, exiting when the first one arrives.
    let (errors_tx, errors_rx) = channel();
    let (refresh_tx, refresh_rx) = channel();

    let ui = Ui::new(errors_tx.clone());

    if let Some(http_config) = &config.http {
        http::run(&http_config, ui.clone(), errors_tx.clone(), refresh_rx)?;
    }

    // Run each action (actions with `on_change` commands will spawn a thread).
    for (name, action) in config.actions.into_iter().flatten() {
        action::run(name, action, &errors_tx, &config.watcher, refresh_tx.clone(), ui.clone())?;
    }
    drop(errors_tx);

    match errors_rx.recv() {
        // There are no thread running, so we can just quit.
        // TODO: use UI
        Err(_) => println!("----- No action has 'on_change' commands, we're done here"),
        // A thread returned an error.
        Ok(e) => return Err(e),
    }

    Ok(())
}
