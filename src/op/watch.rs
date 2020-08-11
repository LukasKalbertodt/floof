use std::{
    time::Duration,
};
use serde::Deserialize;
use crate::{
    Context, Task,
    prelude::*,
};
use super::{Finished, Operation, Operations, Outcome, RunningOperation};


const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watch {
    paths: Vec<String>,
    run: Operations,
    debounce: Option<u32>,
    // TODO: flag to enable polling?
}

impl Watch {
    pub const KEYWORD: &'static str = "watch";
}

impl Operation for Watch {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

        Ok(Box::new(Finished(Outcome::Success)))
    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    fn start(&self, ctx: &Context) -> Result<Box<dyn RunningOperation + '_>> {
    }
}
