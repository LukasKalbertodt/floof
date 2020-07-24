use std::path::Path;
use anyhow::{Context as _, Result};
use structopt::StructOpt;
use crate::{
    args::Args,
    config::Config,
    context::{Context, ContextCreation},
};

mod action;
mod args;
mod config;
mod context;
mod http;
mod step;
mod ui;


fn main() -> Result<()> {
    // Read CLI args.
    let args = Args::from_args();

    // Load configuration (either from specified or default path).
    let default_path = Path::new(config::DEFAULT_FILENAME);
    let config = match &args.config {
        Some(path) => {
            Config::load(path)
                .context(format!("failed to load configuration from '{}'", path.display()))?
        }
        None if default_path.exists() && default_path.is_file() => {
            Config::load(default_path).with_context(|| {
                format!(
                    "failed to load configuration from default location '{}' \
                        (file exists, but is invalid)",
                    config::DEFAULT_FILENAME,
                )
            })?
        }
        None => {
            eprintln!("No configuration found!");
            eprintln!("A `watchboi.toml` has to exist in the current directory or \
                the path to the configuration file has to be given via the \
                `--config`/`-c` argument");
            std::process::exit(1);
        }
    };

    println!("{:#?}", config);


    // Create the context that is given to various threads and other functions.
    let ContextCreation { ctx, reload_requests, errors } = Context::new(config);

    // Start HTTP server if it is requested
    if let Some(http_config) = &ctx.config.http {
        http::run(http_config, reload_requests, ctx.clone())?;
    }

    // Run each action (actions which watch files will spawn a thread and keep
    // running).
    for (name, action) in &ctx.config.actions {
        action::run(&name, &action, &ctx)?;
    }

    // Drop the context to drop all `Sender`s within it.
    let ui = ctx.ui.clone();
    drop(ctx);

    // We collect errors on the main thread, exiting when the first one arrives.
    match errors.recv() {
        // There are no thread running, so we can just quit.
        Err(_) => ui.exiting_no_watcher(),
        // A thread returned an error.
        Ok(e) => return Err(e),
    }

    Ok(())
}
