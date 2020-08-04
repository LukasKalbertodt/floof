//! Command line arguments.

use std::path::PathBuf;
use structopt::StructOpt;


#[derive(StructOpt)]
pub struct Args {
    #[structopt(subcommand)]
    pub cmd: Option<Command>,

    /// Path to the configuration file. If not specified, `watchboi.toml` in the
    /// current directory is used.
    #[structopt(long, short)]
    pub config: Option<PathBuf>,

    /// If this flag is specified, the loaded configuration is printed for
    /// debugging.
    #[structopt(long)]
    pub debug_config: bool,
}

#[derive(StructOpt)]
pub enum Command {
    /// RUn a specific task instead of the default one
    Run {
        /// Name of the task that is supposed to run.
        task: String,
    }
}
