use std::path::Path;
use anyhow::{bail, Context, Result};
use structopt::StructOpt;
use crate::{
    args::Args,
    config::Config,
};

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

    println!("{:#?}", config);

    Ok(())
}
