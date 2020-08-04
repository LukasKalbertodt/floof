#![allow(unused_imports)] // TODO

use structopt::StructOpt;
use crate::{
    prelude::*,
    args::Args,
    cfg::Config,
    // context::{Context, ContextCreation},
};

mod task;
mod args;
mod cfg;
mod op;
mod prelude;
// mod context;
// mod http;
// mod step;
// mod ui;

// We "reexport" some symbols here to make importing them (in other modules)
// easier and to avoid `task::Task` paths.
pub(crate) use crate::{
    task::Task,
    op::{Operation, Operations},
};


fn main() -> Result<()> {
    // Read CLI args.
    let args = Args::from_args();

    // Load configuration (either from specified or default path).
    let config = Config::load(args.config.as_deref())?;

    println!("{:#?}", config);


    // // Create the context that is given to various threads and other functions.
    // let ContextCreation { ctx, reload_requests, errors } = Context::new(config);

    // // Start HTTP server if it is requested
    // if let Some(http_config) = &ctx.config.http {
    //     http::run(http_config, reload_requests, ctx.clone())?;
    // }

    // // Start each task (tasks which watch files will spawn a thread and keep
    // // running).
    // for (name, task) in &ctx.config.tasks {
    //     task::run(&name, &task, &ctx)?;
    // }

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
