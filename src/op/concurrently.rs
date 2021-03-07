use serde::Deserialize;
use crate::{
    Context,
    prelude::*,
};
use super::{Operation, Operations, Outcome};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Concurrently(Operations);

impl Concurrently {
    pub const KEYWORD: &'static str = "concurrently";
}

#[async_trait::async_trait]
impl Operation for Concurrently {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    async fn run(&self, ctx: &Context) -> Result<Outcome> {
        let op_ctx = ctx.fork_op(Self::KEYWORD);

        let mut running_ops = self.0.iter()
            .map(|op| op.run(&op_ctx))
            .collect::<Vec<_>>();

        while !running_ops.is_empty() {
            let (outcome, _, remaining) = futures::future::select_all(running_ops).await;
            running_ops = remaining;

            let outcome = outcome?;
            if !outcome.is_success() {
                return Ok(outcome);
            }
        }

        Ok(Outcome::Success)
    }
}
