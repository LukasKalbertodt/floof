//! Command line arguments.

use std::path::PathBuf;
use structopt::StructOpt;


#[derive(StructOpt)]
pub struct Args {
    /// Path to the configuration file. If not specified, `watchboi.toml` in the
    /// current directory is used.
    #[structopt(long, short)]
    pub config: Option<PathBuf>,
}
