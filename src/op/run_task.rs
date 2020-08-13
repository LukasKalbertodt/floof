use serde::Deserialize;
use crate::{
    Config, Task,
    prelude::*,
};
use super::{Finished, Operation, Operations, Outcome, RunningOperation, ParentKind};

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

    fn start(&self, ctx: &Context) -> Result<Box<dyn RunningOperation>> {
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
