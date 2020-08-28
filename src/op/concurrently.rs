use serde::Deserialize;
use crate::{
    Context,
    prelude::*,
};
use super::{Operation, Operations, Outcome, RunningOperation, OP_NO_OUTCOME_ERROR, BUG_CANCEL_DISCONNECTED};
use crossbeam_channel::Select;

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

    fn start(&self, ctx: &Context) -> Result<RunningOperation> {
        let op_ctx = ctx.fork_op(Self::KEYWORD);
        let operations = self.0.clone();

        let running = RunningOperation::new(&op_ctx, move |ctx, cancel_request| {
            let mut running_ops = operations.iter()
                .map(|op| op.start(ctx))
                .collect::<Result<Vec<_>>>()?;

            while !running_ops.is_empty() {
                let mut select = Select::new();
                for (i, running) in running_ops.iter().enumerate() {
                    let index = select.recv(running.outcome());
                    assert_eq!(i, index);
                }

                let cancel_index = select.recv(&cancel_request);

                let select_op = select.select();
                let index = select_op.index();

                if index == cancel_index {
                    // This operation was cancelled. We don't need the data, but
                    // we should receive from the channel anyway.
                    select_op.recv(&cancel_request).expect(BUG_CANCEL_DISCONNECTED);

                    // Cancel all child operations!
                    for running in &mut running_ops {
                        running.cancel()?;
                    }

                    return Ok(Outcome::Cancelled);
                } else {
                    // One of the operation finished! Receive its outcome.
                    let outcome = select_op.recv(running_ops[index].outcome())
                        .expect(OP_NO_OUTCOME_ERROR)?;

                    if !outcome.is_success() {
                        return Ok(outcome);
                    }

                    running_ops.swap_remove(index);
                }
            }

            Ok(Outcome::Success)
        });

        Ok(running)
    }
}
