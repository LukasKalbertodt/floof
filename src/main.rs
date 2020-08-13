#![allow(unused_imports)] // TODO

use structopt::StructOpt;
use crate::{
    prelude::*,
    cfg::Config,
    // context::{Context, ContextCreation},
};

#[macro_use]
mod ui;

mod task;
mod args;
mod cfg;
mod op;
mod prelude;
mod context;
// mod http;
// mod step;

// We "reexport" some symbols here to make importing them (in other modules)
// easier and to avoid `task::Task` paths.
pub(crate) use crate::{
    args::Args,
    task::Task,
    op::{Operation, Operations},
};


fn main() -> Result<()> {
    // Read CLI args.
    let args = Args::from_args();

    ui::init(&args)?;

    // Load configuration (either from specified or default path).
    let config = Config::load(args.config.as_deref())?;

    if args.debug_config {
        println!("{:#?}", config);
    }

    // Create the context that is given to various threads and other functions.
    let ctx = Context::new(config, args.config.as_deref())?;

    // Start default task.
    match args.cmd {
        None => {
            match ctx.config.tasks.get("default") {
                Some(task) => task.run(&ctx)?,
                None => {
                    eprintln!("No default task defined!");
                    eprintln!("Either define the task 'default' in the configuration or \
                        run `watchboi run <task>` to run a specific task");
                    std::process::exit(1);
                }
            }
        }
        Some(args::Command::Run { task }) => {
            // Make sure that all task names exist before starting anything.
            match ctx.config.tasks.get(&task) {
                Some(task) => task.run(&ctx)?,
                None => {
                    eprintln!("Task '{}' not defined in configuration!", task);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
