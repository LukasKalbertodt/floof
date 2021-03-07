use serde::Deserialize;
use crate::{
    Context,
    prelude::*,
};
use super::{Operation, Outcome};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Copy {
    src: String,
    dst: String,
}

impl Copy {
    pub const KEYWORD: &'static str = "copy";
}

#[async_trait::async_trait]
impl Operation for Copy {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    async fn run(&self, _ctx: &Context) -> Result<Outcome> {
        todo!()
    }
}
