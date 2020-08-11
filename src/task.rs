// use std::{
//     sync::mpsc::{channel, Sender, Receiver, TryRecvError, RecvTimeoutError},
//     thread, path::Path, time::{Duration, Instant},
// };

// use notify::{Watcher, RecursiveMode};

use crate::{
    Operations,
    prelude::*,
    context::FrameKind,
};

use crate::Operation;


#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub operations: Operations,
}

impl Task {
    pub fn validate(&self) -> Result<()> {
        for op in &self.operations {
            op.validate()?;
        }

        Ok(())
    }

    pub fn run(&self, ctx: &Context) -> Result<()> {
        let ctx = ctx.fork(FrameKind::Task(self.name.clone()));
        verbose!(- [ctx] - "Starting task");

        for op in &self.operations {
            let outcome = op.run(&ctx).with_context(|| {
                // TODO: nicer output of the operation
                format!("failed to run operation for task '{}':\n{:#?}", self.name, op)
            })?;

            if outcome.is_failure() {
                verbose!(
                    - [ctx] - "'{}' operation failed → stopping (no further operations of \
                        this task are ran)",
                    op.keyword(),
                );
                break;
            }
        }

        verbose!(- [ctx] - "Finished running all operations of task", self.name);

        Ok(())
    }
}
