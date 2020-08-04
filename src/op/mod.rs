use std::fmt;
use anyhow::Result;


mod copy;
mod command;
mod http;
mod watch;

pub use self::{
    copy::Copy,
    command::Command,
    http::Http,
    watch::Watch,
};


pub type Operations = Vec<Box<dyn Operation>>;


pub trait Operation: fmt::Debug + 'static + Send + Sync {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}
