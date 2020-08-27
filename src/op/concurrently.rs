use serde::Deserialize;
use crate::{
    Context,
    prelude::*,
};
use super::{Operation, Operations, Outcome, RunningOperation};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Concurrently(Operations);

impl Concurrently {
    pub const KEYWORD: &'static str = "concurrently";
}

impl Operation for Concurrently {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    fn start(&self, ctx: &Context) -> Result<Box<dyn RunningOperation + '_>> {
        let mut running = Vec::new();

        let op_ctx = ctx.fork_op("concurrently");
        for op in &self.0 {
            running.push(op.start(&op_ctx)?);
        }

        // let (tx, rx) = channel();
        // let handle = thread::spawn(move || {
        //     match rx.try_recv() {
        //         Ok(_) => {}
        //         Err(_) => {}
        //     }
        // });

        Ok(Box::new(RunningConcurrently {
            operations: running,
            any_failure: false,
        }))
    }
}
struct RunningConcurrently<'a> {
    operations: Vec<Box<dyn RunningOperation + 'a>>,
    any_failure: bool,
}

impl RunningOperation for RunningConcurrently<'_> {
    fn finish(&mut self, ctx: &Context) -> Result<Outcome> {
        for op in &mut self.operations {
            let outcome = op.finish(ctx)?;
            self.any_failure |= outcome.is_failure();
        }

        Ok(if self.any_failure { Outcome::Failure } else { Outcome::Success })
    }
    fn try_finish(&mut self, ctx: &Context) -> Result<Option<Outcome>> {
        // In the future, we can use `Vec::drain_filter` here.
        let mut finished = Vec::new();
        for (i, op) in &mut self.operations.iter_mut().enumerate() {
            if let Some(outcome) = op.try_finish(ctx)? {
                finished.push(i);
                self.any_failure |= outcome.is_failure();
            }
        }

        // Remove all operations that have finished. We remove indices from high
        // to low to not invalidate any indices.
        for i in finished.into_iter().rev() {
            self.operations.swap_remove(i);
        }

        if self.operations.is_empty() {
            Ok(Some(if self.any_failure { Outcome::Failure } else { Outcome::Success }))
        } else {
            Ok(None)
        }
    }
    fn cancel(&mut self) -> Result<()> {
        for mut op in self.operations.drain(..) {
            op.cancel()?;
        }

        Ok(())
    }
}
