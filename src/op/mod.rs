use std::fmt;
use anyhow::Result;


mod copy;
mod command;
mod http;

pub use self::{
    copy::Copy,
    command::Command,
    http::Http,
};

pub trait Operation: fmt::Debug + 'static + Send + Sync {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}
