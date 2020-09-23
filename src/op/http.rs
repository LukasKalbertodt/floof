use anyhow::{bail, Error};
use std::{
    convert::TryFrom,
    net::{SocketAddr, ToSocketAddrs}, fmt, future::Future, thread, sync::Arc,
};
use crossbeam_channel::Sender;
use serde::Deserialize;
use crate::{
    Context,
    prelude::*,
};
use super::{Operation, RunningOperation, Outcome, BUG_CANCEL_DISCONNECTED};


/// An HTTP server able to function as a reverse proxy or static file server.
/// Can inject JS code into the response to reload the page whenever a `reload:`
/// operation is executed.
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

    fn start(&self, ctx: &Context) -> Result<RunningOperation> {
        use crate::http::{Server, Mode};

        let default_addr: SocketAddr = "127.0.0.1:8030".parse().unwrap();
        let default_ws_addr: SocketAddr = "127.0.0.1:8031".parse().unwrap();

        // Prepare configuration for dev server
        let bind = self.addr.map_or(default_addr, |a| a.0);
        let bind_control = self.ws_addr.map_or(default_ws_addr, |a| a.0);
        let mode = match (self.proxy, &self.serve) {
            // TODO: actually check that in validation
            (None, None) | (Some(_), Some(_)) => panic!("bug: invalid config"),
            (Some(proxy_target), None) => Mode::RevProxy { target: proxy_target.0 },
            (None, Some(_)) => panic!("file server not implemented yet :("),
        };
        let callback_ctx = ctx.clone();
        let callback = Arc::new(move |event| {
            match event {
                crate::http::Event::Reload => {
                    msg!(reload [callback_ctx]["http"] "Reloading all active sessions");

                }
                _ => {}
            }
        });
        let config = crate::http::Config { mode, bind, bind_control, callback };

        // Setup communication for reload requests.
        let (reload_tx, reload_rx) = crossbeam_channel::unbounded();
        ctx.top_frame.insert_var(Reloader(reload_tx));


        let running = RunningOperation::start(ctx, move |ctx, cancel_request| {
            #[tokio::main]
            async fn runner(f: impl Future<Output = Result<(), crate::http::Error>>) -> Result<()> {
                f.await?;
                Ok(())
            }


            let (server, listen) = Server::new(config)?;

            let (server_done_tx, server_done_rx) = crossbeam_channel::bounded(1);
            thread::spawn(move || {
                let res = runner(listen);
                server_done_tx.send(res.map(|_| Outcome::Cancelled)).unwrap();
            });

            msg!(- [ctx]["http"] "Listening on {$yellow+intense+bold}http://{}{/$}", bind);

            let unexpected_end_err = "server thread unexpectedly stopped";
            loop {
                crossbeam_channel::select! {
                    recv(reload_rx) -> res => {
                        res.expect("reloader unexpectedly dropped");
                        server.reload();
                    },
                    recv(server_done_rx) -> outcome => {
                        let _ = outcome.expect(unexpected_end_err);
                        panic!("bug: HTTP server should never stop on its own...");
                    },
                    recv(cancel_request) -> res => {
                        res.expect(BUG_CANCEL_DISCONNECTED);
                        verbose!(- [ctx]["http"] "cancelling HTTP server...");
                        server.stop();
                        return server_done_rx.recv().expect(unexpected_end_err);
                    },
                }
            }
        });

        Ok(running)
    }
}

#[derive(Debug, Clone)]
struct Reloader(Sender<()>);

/// Operation to reload the browser sessions of the nearest `http` instance.
#[derive(Debug, Clone, Deserialize)]
pub struct Reload;

impl Reload {
    pub const KEYWORD: &'static str = "reload";
}

impl Operation for Reload {
    fn keyword(&self) -> &'static str {
        Self::KEYWORD
    }

    fn dyn_clone(&self) -> Box<dyn Operation> {
        Box::new(self.clone())
    }

    fn start(&self, ctx: &Context) -> Result<RunningOperation> {
        verbose!(- [ctx]["reload"] "Sent reload request");
        match ctx.get_closest_var::<Reloader>() {
            Some(reloader) => {
                reloader.0.send(()).expect("bug: reload channel in reloader has hung up");
                Ok(RunningOperation::finished(Outcome::Success))
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
