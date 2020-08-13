// use std::{
//     sync::mpsc::{channel, Sender, Receiver, TryRecvError, RecvTimeoutError},
//     thread, path::Path, time::{Duration, Instant},
// };

// use notify::{Watcher, RecursiveMode};

use crate::{
    Operation, Operations,
    prelude::*,
    op::Outcome,
    context::FrameKind,
};


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

    pub fn run(&self, ctx: &Context) -> Result<Outcome> {
        let ctx = ctx.fork_task(&self.name);
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
                return Ok(Outcome::Failure)
            }
        }

        verbose!(- [ctx] - "Finished running all operations of task", self.name);

        Ok(Outcome::Success)
    }
}
