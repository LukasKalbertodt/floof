use structopt::StructOpt;
use crate::{
    prelude::*,
    cfg::Config,
};

#[macro_use]
mod ui;

mod task;
mod args;
mod cfg;
mod op;
mod prelude;
mod context;

// We "reexport" some symbols here to make importing them (in other modules)
// easier and to avoid `task::Task` paths.
pub(crate) use crate::{
    args::Args,
    task::Task,
    op::{Operation, Operations},
};


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
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
    let exit_code = match args.cmd {
        None => {
            match ctx.config.tasks.get("default") {
                Some(task) => task.run(&ctx).await?.to_exit_code(),
                None => {
                    eprintln!("No default task defined!");
                    eprintln!("Either define the task 'default' in the configuration or \
                        run `floof run <task>` to run a specific task");
                    1
                }
            }
        }
        Some(args::Command::Run { task }) => {
            // Make sure that all task names exist before starting anything.
            match ctx.config.tasks.get(&task) {
                Some(task) => task.run(&ctx).await?.to_exit_code(),
                None => {
                    eprintln!("Task '{}' not defined in configuration!", task);
                    1
                }
            }
        }
    };

    std::process::exit(exit_code);
}
