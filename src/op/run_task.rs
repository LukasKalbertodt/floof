use serde::Deserialize;
use crate::{
    Config,
    prelude::*,
};
use super::{Operation, Outcome, ParentKind};


#[derive(Debug, Clone, Deserialize)]
pub struct RunTask(String);

impl RunTask {
    pub const KEYWORD: &'static str = "run-task";
}

#[async_trait::async_trait]
impl Operation for RunTask {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    async fn run(&self, ctx: &Context) -> Result<Outcome> {
        let task_name = self.0.clone();
        let task = &ctx.config.tasks[&task_name];

        let op_ctx = ctx.fork_task(&task_name);
        for op in &task.operations {
            let outcome = op.run(&op_ctx).await?;
            if !outcome.is_success() {
                return Ok(outcome);
            }
        }

        Ok(Outcome::Success)
    }

    fn validate(&self, _parent: ParentKind<'_>, config: &Config) -> Result<()> {
        // TODO: maybe disallow recursion?

        if !config.tasks.contains_key(&self.0) {
            bail!("task '{}' does not exist", self.0);
        }

        Ok(())
    }
}
