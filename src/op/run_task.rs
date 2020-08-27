use serde::Deserialize;
use crate::{
    Config,
    prelude::*,
};
use super::{Finished, Operation, Outcome, RunningOperation, ParentKind};

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

    fn start(&self, ctx: &Context) -> Result<Box<dyn RunningOperation + '_>> {
        let task = &ctx.config.tasks[&self.0];

        // TODO: run this in new thread and make cancelable. This is a bit
        // tricky though.
        let outcome = task.run(ctx)?;

        Ok(Box::new(Finished(outcome)))
    }

    fn validate(&self, _parent: ParentKind<'_>, config: &Config) -> Result<()> {
        // TODO: maybe disallow recursion?

        if !config.tasks.contains_key(&self.0) {
            bail!("task '{}' does not exist", self.0);
        }

        Ok(())
    }
}


struct Running<'a> {
    task_name: &'a str,
    current: Option<Box<dyn RunningOperation>>,
    already_finished: usize,
}

impl Running<'_> {

}

impl RunningOperation for Running<'_> {
    fn finish(&mut self, ctx: &Context) -> Result<Outcome> {
        if let Some(op) = &mut self.current {
            let outcome = op.finish(ctx)?;
            self.already_finished += 1;
            self.current = None;

            if outcome.is_failure() {
                return Ok(outcome);
            }
        }

        let task = &ctx.config.tasks[self.task_name];
        for op in &task.operations[self.already_finished..] {
            let outcome = op.run(ctx)?;

            if outcome.is_failure() {
                return Ok(outcome);
            }
        }

        Ok(Outcome::Success)
    }
    fn try_finish(&mut self, ctx: &Context) -> Result<Option<Outcome>> {
        todo!()
    }
    fn cancel(&mut self) -> Result<()> {
        if let Some(op) = &mut self.current {
            op.cancel()?;
        }
        Ok(())
    }
}
