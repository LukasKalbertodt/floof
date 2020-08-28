use serde::Deserialize;
use crate::{
    Config,
    prelude::*,
};
use super::{Operation, Outcome, RunningOperation, ParentKind, OP_NO_OUTCOME_ERROR};

#[derive(Debug, Clone, Deserialize)]
pub struct RunTask(String);

impl RunTask {
    pub const KEYWORD: &'static str = "run-task";
}

impl Operation for RunTask {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    fn start(&self, ctx: &Context) -> Result<RunningOperation> {
        let task_name = self.0.clone();
        let running = RunningOperation::start(ctx, move |ctx, cancel_request| {
            let task = &ctx.config.tasks[&task_name];

            let op_ctx = ctx.fork_task(&task_name);
            for op in &task.operations {
                let mut running = op.start(&op_ctx)?;
                crossbeam_channel::select! {
                    recv(running.outcome()) -> outcome => {
                        let outcome = outcome.expect(OP_NO_OUTCOME_ERROR)?;
                        if !outcome.is_success() {
                            return Ok(outcome);
                        }
                    }
                    recv(cancel_request) -> result => {
                        result.expect(OP_NO_OUTCOME_ERROR);
                        running.cancel()?;
                        return Ok(Outcome::Cancelled)
                    }
                }
            }

            Ok(Outcome::Success)

        });

        Ok(running)
    }

    fn validate(&self, _parent: ParentKind<'_>, config: &Config) -> Result<()> {
        // TODO: maybe disallow recursion?

        if !config.tasks.contains_key(&self.0) {
            bail!("task '{}' does not exist", self.0);
        }

        Ok(())
    }
}
