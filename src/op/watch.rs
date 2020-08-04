use std::{
    time::Duration,
};
use serde::Deserialize;
use super::{Operation, Operations};


const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watch {
    paths: Vec<String>,
    run: Operations,
    debounce: Option<u32>,
    // TODO: flag to enable polling?
}

impl Operation for Watch {

}
