//! Command line arguments.

use std::path::PathBuf;
use structopt::StructOpt;
use termcolor::ColorChoice;


#[derive(StructOpt)]
pub struct Args {
    #[structopt(subcommand)]
    pub cmd: Option<Command>,

    /// Path to the configuration file. If not specified, `floof.yaml` in the
    /// current directory is used.
    #[structopt(long, short)]
    pub config: Option<PathBuf>,

    /// If this flag is specified, the loaded configuration is printed for
    /// debugging.
    #[structopt(long)]
    pub debug_config: bool,

    /// Verbosity level: `-v` or `-vv` allowed.
    #[structopt(short, parse(from_occurrences))]
    pub verbose: u8,

    /// Controls the use of colors: 'never', 'auto' or 'always'.
    #[structopt(long, default_value = "auto", parse(try_from_str = parse_color_choice))]
    pub color: ColorChoice,
}

#[derive(StructOpt)]
pub enum Command {
    /// Run a specific task instead of the default one
    Run {
        /// Name of the task that is supposed to run.
        task: String,
    }
}

fn parse_color_choice(input: &str) -> Result<ColorChoice, String> {
    match input {
        "never" => Ok(ColorChoice::Never),
        "auto" => Ok(ColorChoice::Auto),
        "always" => Ok(ColorChoice::Always),
        _ => Err(format!("'{}' is not a valid value for parameter --color", input)),
    }
}
