//! Command line arguments.

use std::path::PathBuf;
use structopt::StructOpt;


#[derive(StructOpt)]
pub struct Args {
    /// Path to the configuration file. If not specified, `watchboi.toml` in the
    /// current directory is used.
    #[structopt(long, short)]
    pub config: Option<PathBuf>,

    /// If this flag is specified, the loaded configuration is printed for
    /// debugging.
    #[structopt(long)]
    pub debug_config: bool,
}
