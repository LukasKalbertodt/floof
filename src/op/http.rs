use anyhow::{bail, Error};
use std::{
    convert::TryFrom,
    fmt,
    net::{SocketAddr, ToSocketAddrs},
};
use serde::Deserialize;
use crate::{
    Context,
    prelude::*,
};
use super::{Operation, Outcome};


/// An HTTP server able to function as a reverse proxy or static file server.
/// Can inject JS code into the response to reload the page whenever a `reload:`
/// operation is executed.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Http {
    proxy: Option<String>,
    serve: Option<String>,

    addr: Option<Addr>,
}

impl Http {
    pub const KEYWORD: &'static str = "http";
}

#[async_trait::async_trait]
impl Operation for Http {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    async fn run(&self, ctx: &Context) -> Result<Outcome> {
        let default_addr: SocketAddr = "127.0.0.1:8030".parse().unwrap();

        let bind_addr = self.addr.map_or(default_addr, |a| a.0);
        let builder = penguin::Server::bind(bind_addr);

        // Prepare configuration for dev server
        let builder = match (&self.proxy, &self.serve) {
            // TODO: actually check that in validation
            (None, None) | (Some(_), Some(_)) => panic!("bug: invalid config"),
            (Some(proxy_target), None) => {
                let target = proxy_target.parse().context("invalid proxy target")?;
                builder.proxy(target)
            }
            (None, Some(path)) => builder.add_mount("/", path).unwrap(),
        };
        let (server, controller) = builder.build()?;


        //msg!(reload [callback_ctx]["http"] "Reloading all active sessions");

        // Setup communication for reload requests.
        ctx.top_frame.insert_var(Reloader(controller));

        msg!(- [ctx]["http"] "Listening on {$yellow+intense+bold}http://{}{/$}", bind_addr);
        server.await?;

        // TODO: actually stop server when requested

        Ok(Outcome::Success)
    }
}

#[derive(Debug, Clone)]
struct Reloader(penguin::Controller);

/// Operation to reload the browser sessions of the nearest `http` instance.
#[derive(Debug, Clone, Deserialize)]
pub struct Reload;

impl Reload {
    pub const KEYWORD: &'static str = "reload";
}

#[async_trait::async_trait]
impl Operation for Reload {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    async fn run(&self, ctx: &Context) -> Result<Outcome> {
        verbose!(- [ctx]["reload"] "Sent reload request");
        match ctx.get_closest_var::<Reloader>() {
            Some(reloader) => {
                reloader.0.reload();
                Ok(Outcome::Success)
            }
            None => {
                bail!("`reload` operation started, but no HTTP server registered in this \
                    context or any of its parents");
            }
        }
    }
}

/// Wrapper around `SocketAddr` that nicely deserializes.
#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
struct Addr(SocketAddr);

impl TryFrom<String> for Addr {
    type Error = Error;

    fn try_from(src: String) -> Result<Self, Self::Error> {
        let addr = src.to_socket_addrs()?.next()
            .ok_or(anyhow!("expected one address, but parsing '{}' returned none", &src))?;
        Ok(Self(addr))
    }
}

impl fmt::Debug for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}


// async fn wait_until_socket_open(target: SocketAddr) -> bool {
//     const POLL_PERIOD: Duration = Duration::from_millis(20);
//     const PORT_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

//     let start_wait = Instant::now();
//     while start_wait.elapsed() < PORT_WAIT_TIMEOUT {
//         let before_connect = Instant::now();
//         if let Ok(Ok(_)) = tokio::time::timeout(POLL_PERIOD, TcpStream::connect(&target)).await {
//             return true;
//         }

//         if let Some(remaining) = POLL_PERIOD.checked_sub(before_connect.elapsed()) {
//             thread::sleep(remaining);
//         }
//     }

//     // TODO: call a callback here

//     false
// }
