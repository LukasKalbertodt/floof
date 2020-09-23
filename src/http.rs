#![allow(unused_imports)] // TODO
use std::{
    future::Future,
    io,
    net::SocketAddr,
    path::PathBuf,
    sync::{Mutex, Arc},
    thread,
    time::{Duration, Instant},
};
use flume::{Sender, Receiver};
use hyper::{
    Body, Client, Request, Response, Server as HyperServer, Uri, StatusCode,
    header,
    service::{make_service_fn, service_fn}
};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::WebSocketStream;


#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("hyper HTTP server error: {0}")]
    Hyper(#[from] hyper::Error),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Clone)]
pub struct Config {
    /// What this server should do.
    pub mode: Mode,

    /// The address that this server should bind to (i.e. basically what you
    /// have to open in the browser).
    pub bind: SocketAddr,

    /// The address the control functionality is bound to. Over this port,
    /// WebSocket connections for auto-reload are handled and certain actions
    /// can be triggered.
    pub bind_control: SocketAddr,

    /// A callback that is called whenever anything notable happens. Also see
    /// [`Event`].
    pub callback: Arc<dyn Fn(Event) + Send + Sync>,
}

#[derive(Debug, Clone)]
pub enum Mode {
    /// Makes this server function as a reverse proxy. HTTP request are verbatim
    /// forwarded to the given target address and its response will be used. The
    /// response might be slightly altered though, for example by injecting JS
    /// code which is needed for automatic reloading.
    RevProxy {
        target: SocketAddr,
    },

    /// Makes this server function as a simple static file server.
    FileServer {
        root: PathBuf,
    },
}

/// Things that can happen when using this dev server. This is mainly used in
/// the callback.
#[non_exhaustive]
pub enum Event {
    /// The main HTTP server started listening and is now ready to receive
    /// requests.
    MainServerStarted,

    /// The control HTTP server started listening and is now ready to receive
    /// requests.
    ControlServerStarted,

    /// A reload command is sent via WS to all active sessions. This is only
    /// fired after the socket has been waited on.
    Reload,
}

pub struct Server {
    cancel: Sender<()>,
    reload: Sender<()>,
}

impl Server {
    pub fn new(config: Config) -> Result<(Self, impl Future<Output = Result<(), Error>>), Error> {
        let (cancel_tx, cancel_rx) = flume::bounded(0);
        let (reload_tx, reload_rx) = flume::unbounded();

        let listen = run(config, reload_rx, cancel_rx);
        let server = Self {
            cancel: cancel_tx,
            reload: reload_tx,
        };

        Ok((server, listen))
    }

    pub fn reload(&self) {
        self.reload.send(())
            .expect("bug: server thread has unexpectedly ended");
    }

    pub fn stop(self) {
        self.cancel.send(())
            .expect("bug: server thread has unexpectedly ended");
    }
}



async fn run(config: Config, reload: Receiver<()>, cancel: Receiver<()>) -> Result<(), Error> {
    let config_clone = config.clone();
    let (res0, res1) = tokio::try_join!(
        tokio::spawn(run_http_server(config, cancel)),
        tokio::spawn(run_ws_server(config_clone, reload)),
    ).expect("a task was cancelled or panicked");

    res0.and(res1)
}

async fn run_http_server(config: Config, cancel: Receiver<()>) -> Result<(), Error> {
    let service = match config.mode {
        Mode::FileServer { .. } => {
            panic!("Fileserver not yet implemented :-(");
        }
        Mode::RevProxy { target } => {
            let bind_control = config.bind_control;
            make_service_fn(move |_| {
                async move {
                    Ok::<_, hyper::Error>(service_fn(move |req| {
                        rev_proxy(req, target, bind_control)
                    }))
                }
            })
        }
    };

    let server = hyper::Server::bind(&config.bind).serve(service);
    let server = server.with_graceful_shutdown(async move {
        // We don't care if the other side of the channel hung up or not, we
        // will quit either way.
        let _ = cancel.recv_async().await;
    });

    (config.callback)(Event::MainServerStarted);
    server.await?;

    Ok(())
}


async fn rev_proxy(
    mut req: Request<Body>,
    target: SocketAddr,
    control_addr: SocketAddr,
) -> Result<Response<Body>, Error> {
    let uri = Uri::builder()
        .scheme("http")
        .authority(target.to_string().as_str())
        .path_and_query(req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or(""))
        .build()
        .expect("failed to build URI");
    *req.uri_mut() = uri.clone();

    let client = Client::new();
    let response = match client.request(req).await {
        Ok(response) => {
            let content_type = response.headers().get(header::CONTENT_TYPE);
            if content_type.is_some() && content_type.unwrap().as_ref().starts_with(b"text/html") {
                let (parts, body) = response.into_parts();
                let body = hyper::body::to_bytes(body).await?;

                let new_body = inject_into(&body, control_addr);
                let new_len = new_body.len();
                let new_body = Body::from(new_body);

                let mut response = Response::from_parts(parts, new_body);
                if let Some(content_len) = response.headers_mut().get_mut(header::CONTENT_LENGTH) {
                    *content_len = new_len.into();
                }
                response
            } else {
                response
            }
        }
        Err(e) => {
            let msg = format!("Failed to reach {}\n\n{}", uri, e);
            let html = format!(
                "<html>\n  \
                  <head><title>Floof can't reach proxy target</title></head>\n  \
                  <body>\n    \
                    <h1>Floof failed to connect to the proxy target :(</h1>\n    \
                    <pre>{}</pre>\n    \
                    {}\n  \
                  </body>\n\
                </html>",
                msg,
                reload_script(control_addr),
            );

            Response::builder()
                // TODO: sometimes this should be 504 GATEWAY TIMEOUT
                .status(StatusCode::BAD_GATEWAY)
                .header("Content-Type", "text/html")
                .body(html.into())
                .unwrap()
        }
    };

    Ok(response)
}

fn reload_script(control_addr: SocketAddr) -> String {
    const JS_CODE: &str = include_str!("inject.js");

    let js = JS_CODE.replace("INSERT_PORT_HERE_KTHXBYE", &control_addr.port().to_string());
    format!("<script>\n{}</script>", js)
}

fn inject_into(input: &[u8], control_addr: SocketAddr) -> Vec<u8> {
    let mut body_close_idx = None;
    let mut inside_comment = false;
    for i in 0..input.len() {
        let rest = &input[i..];
        if !inside_comment && rest.starts_with(b"</body>") {
            body_close_idx = Some(i);
        } else if !inside_comment && rest.starts_with(b"<!--") {
            inside_comment = true;
        } else if inside_comment && rest.starts_with(b"-->") {
            inside_comment = false;
        }
    }

    // If we haven't found a closing body tag, we just insert our JS at the very
    // end.
    let insert_idx = body_close_idx.unwrap_or(input.len());
    let mut out = input[..insert_idx].to_vec();
    out.extend_from_slice(reload_script(control_addr).as_bytes());
    out.extend_from_slice(&input[insert_idx..]);
    out
}

async fn run_ws_server(
    config: Config,
    reload: Receiver<()>,
) -> Result<(), Error> {
    let sockets = Arc::new(Mutex::new(Vec::<WebSocketStream<_>>::new()));

    // Start thread that listens for incoming refresh requests.
    {
        let proxy_target = match config.mode {
            Mode::RevProxy { target } => Some(target),
            Mode::FileServer { .. } => None,
        };
        let sockets = sockets.clone();
        let callback = config.callback.clone();
        tokio::spawn(async move {
            while let Ok(_) = reload.recv_async().await {
                if let Some(target) = proxy_target {
                    if !wait_until_socket_open(target).await {
                        // Socket did not become available, so we just ignore
                        // this reload request.
                        continue;
                    }
                }

                // All connections are closed when the `TcpStream` inside those
                // `WebSocket` is dropped.
                callback(Event::Reload);
                sockets.lock().unwrap().clear();
            }

            // Receiving failed because the channel was closed. This is fine, we
            // just stop now.
        });
    }

    // Listen for new WS connections, accept them and push them in the vector.
    let mut server = TcpListener::bind(config.bind_control).await?;
    (config.callback)(Event::ControlServerStarted);

    while let Ok((raw_stream, _addr)) = server.accept().await {
        let sockets = sockets.clone();
        tokio::spawn(async move {
            let ws = tokio_tungstenite::accept_async(raw_stream).await;
            match ws {
                Err(_) => {
                    // TODO: on error callback or sth
                }
                Ok(ws) => {
                    sockets.lock().unwrap().push(ws);
                }
            }
        });
    }

    Ok(())
}

async fn wait_until_socket_open(target: SocketAddr) -> bool {
    const POLL_PERIOD: Duration = Duration::from_millis(20);
    const PORT_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

    let start_wait = Instant::now();
    while start_wait.elapsed() < PORT_WAIT_TIMEOUT {
        let before_connect = Instant::now();
        if let Ok(Ok(_)) = tokio::time::timeout(POLL_PERIOD, TcpStream::connect(&target)).await {
            return true;
        }

        if let Some(remaining) = POLL_PERIOD.checked_sub(before_connect.elapsed()) {
            thread::sleep(remaining);
        }
    }

    // TODO: call a callback here

    false
}
