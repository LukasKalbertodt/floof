use std::fmt;
use anyhow::Result;


mod copy;
mod command;

pub use self::{
    copy::Copy,
    command::Command,
};

pub trait Operation: fmt::Debug + 'static + Send + Sync {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}
