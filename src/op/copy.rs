use serde::Deserialize;
use crate::{
    Context, Task,
    prelude::*,
};
use super::{Finished, Operation, Operations, Outcome, RunningOperation};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Copy {
    src: String,
    dst: String,
}

impl Operation for Copy {
    fn start(&self, task: &Task, ctx: &Context) -> Result<Box<dyn RunningOperation>> {
        Ok(Box::new(Finished(Outcome::Success)))
    }
}
