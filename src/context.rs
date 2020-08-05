use std::{
    sync::{
        Arc,
        mpsc::{self, Sender, Receiver},
    },
    thread,
};
use crate::{
    prelude::*,
    cfg::Config,
};


/// Contains central information and synchronization utilities that most parts
/// of the program need access to.
#[derive(Clone)]
pub struct Context {
    pub config: Arc<Config>,
    errors: Sender<Error>,
}

/// Helper struct for `Context::new`.
pub struct ContextCreation {
    pub ctx: Context,
    pub errors: Receiver<Error>,
}

impl Context {
    /// Creates a new context.
    pub fn new(config: Config) -> ContextCreation {
        let (errors_tx, errors_rx) = mpsc::channel();

        ContextCreation {
            ctx: Self {
                config: Arc::new(config),
                errors: errors_tx,
            },
            errors: errors_rx,
        }
    }

    /// Send the given error to the main thread. The main thread will then print
    /// the error and terminate, terminating all other threads with as well.
    pub fn report_error(&self, e: Error) {
        // We ignore the result here. It is only `Err` if the channel has hung
        // up, which means the main thread has ended. But it isn't possible that
        // the main thread ended but the child threads did not.
        let _ = self.errors.send(e);
    }

    /// Spawns a thread that executes the given function `f`. If the function
    /// produces an error, `self.report_error` is called with said error.
    pub fn spawn_thread(&self, f: impl FnOnce(&Self) -> Result<()> + Send + 'static) {
        let ctx = self.clone();
        thread::spawn(move || {
            if let Err(e) = f(&ctx) {
                ctx.report_error(e);
            }
        });
    }
}
