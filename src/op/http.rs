use anyhow::{bail, Error};
use std::{
    convert::TryFrom,
    net::{SocketAddr, ToSocketAddrs}, fmt,
};
use serde::Deserialize;
use crate::{
    Context,
    prelude::*,
};
use super::{Operation, RunningOperation};


#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Http {
    proxy: Option<Addr>,
    serve: Option<String>,

    addr: Option<Addr>,

    #[serde(rename = "ws-addr")]
    ws_addr: Option<Addr>,
}

impl Http {
    pub const KEYWORD: &'static str = "http";
}

impl Operation for Http {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    fn start(&self, _ctx: &Context) -> Result<RunningOperation> {
        todo!()
    }
}

#[derive(Clone, Deserialize)]
#[serde(try_from = "String")]
struct Addr(SocketAddr);

impl TryFrom<String> for Addr {
    type Error = Error;

    fn try_from(src: String) -> Result<Self, Self::Error> {
        let addrs = src.to_socket_addrs()?.collect::<Vec<_>>();
        if addrs.len() != 1 {
            bail!("expected one address, but found {}", addrs.len());
        }

        Ok(Self(addrs[0]))
    }
}

impl fmt::Debug for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
