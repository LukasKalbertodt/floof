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

    msg!(info - - "Starting up watchboi :)");
    msg!(eye ["frontend"]["watch"] "Watching {[yellow]}", "Cargo.toml");
    verbose!(none - - "buhu");


    // Create the context that is given to various threads and other functions.
    let context::ContextCreation { ctx, errors } = Context::new(config);

    // Start default task.
    match args.cmd {
        None => {
            match ctx.config.tasks.get("default") {
                Some(task) => task.run(&ctx)?,
                None => {
                    eprintln!("No default task defined!");
                    eprintln!("Either define the task 'default' in the configuration or \
                        run `watchboi run <tasks...>` to run specific tasks");
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


    // // Drop the context to drop all `Sender`s within it.
    // let ui = ctx.ui.clone();
    // drop(ctx);

    // // We collect errors on the main thread, exiting when the first one arrives.
    // match errors.recv() {
    //     // There are no thread running, so we can just quit.
    //     Err(_) => ui.exiting_no_watcher(),
    //     // A thread returned an error.
    //     Ok(e) => return Err(e),
    // }

    Ok(())
}
