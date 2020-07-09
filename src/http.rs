use std::{
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{mpsc::{self, Receiver, Sender}, Arc, Mutex},
    thread, time::{Duration, Instant},
};
use anyhow::{bail, Result};
use hyper::{
    Body, Client, Request, Response, Server, Uri, StatusCode,
    header,
    service::{make_service_fn, service_fn}
};
use tungstenite::WebSocket;

use crate::{
    config,
    context::Context,
};


pub fn run(config: &config::Http, reload_requests: Receiver<String>, ctx: Context) -> Result<()> {
    let (init_tx, init_rx) = mpsc::channel();

    // Start the HTTP server thread.
    {
        let config = config.clone();
        let init_tx = init_tx.clone();
        ctx.spawn_thread(move |ctx| run_server(&config, init_tx, ctx));
    }

    // Potentially start the thread serving the websocket connection for
    // auto_reloads.
    if ctx.config.auto_reload() {
        let config = config.clone();
        ctx.spawn_thread(move |ctx| serve_ws(&config, reload_requests, init_tx, ctx));
    }

    // Wait for all threads to have initialized
    let waiting_for = if ctx.config.auto_reload() { 2 } else { 1 };
    init_rx.iter().take(waiting_for).last();

    Ok(())
}

#[tokio::main]
pub async fn run_server(
    config: &config::Http,
    init_done: Sender<()>,
    ctx: &Context,
) -> Result<()> {
    let addr = config.addr();
    let ws_addr = config.ws_addr();

    let service = if let Some(proxy_target) = config.proxy {
        let auto_reload = ctx.config.auto_reload();

        make_service_fn(move |_| {
            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    proxy(req, proxy_target, ws_addr, auto_reload)
                }))
            }
        })
    } else {
        bail!("bug: invalid http config");
    };

    let server = Server::bind(&addr).serve(service);
    ctx.ui.listening(&addr);
    init_done.send(()).unwrap();

    server.await?;

    Ok(())
}


async fn proxy(
    mut req: Request<Body>,
    target: SocketAddr,
    ws_addr: SocketAddr,
    auto_reload: bool,
) -> Result<Response<Body>> {
    let uri = Uri::builder()
        .scheme("http")
        .authority(target.to_string().as_str())
        .path_and_query(req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or(""))
        .build()
        .expect("failed to build URI");
    *req.uri_mut() = uri.clone();

    let client = Client::new();
    let response = match client.request(req).await {
        Ok(response) if !auto_reload => response,
        Ok(response) => {
            let content_type = response.headers().get(header::CONTENT_TYPE);
            if content_type.is_some() && content_type.unwrap().as_ref().starts_with(b"text/html") {
                let (parts, body) = response.into_parts();
                let body = hyper::body::to_bytes(body).await?;

                let new_body = inject_into(&body, ws_addr);
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
                  <head><title>Watchboi can't reach proxy target</title></head>\n  \
                  <body>\n    \
                    <h1>Watchboi failed to connect to the proxy target :(</h1>\n    \
                    <pre>{}</pre>\n    \
                    {}\n  \
                  </body>\n\
                </html>",
                msg,
                reload_script(ws_addr),
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

fn reload_script(ws_addr: SocketAddr) -> String {
    const JS_CODE: &str = include_str!("inject.js");

    let js = JS_CODE.replace("INSERT_PORT_HERE_KTHXBYE", &ws_addr.port().to_string());
    format!("<script>\n{}</script>", js)
}

fn inject_into(input: &[u8], ws_addr: SocketAddr) -> Vec<u8> {
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
    out.extend_from_slice(reload_script(ws_addr).as_bytes());
    out.extend_from_slice(&input[insert_idx..]);
    out
}

fn serve_ws(
    config: &config::Http,
    reload_requests: Receiver<String>,
    init_done: Sender<()>,
    ctx: &Context,
) -> Result<()> {
    let sockets = Arc::new(Mutex::new(Vec::<WebSocket<_>>::new()));

    // Start thread that listens for incoming refresh requests.
    {
        let proxy_target = config.proxy;
        let sockets = sockets.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            for action_name in reload_requests {
                if let Some(target) = proxy_target {
                    wait_until_socket_open(target, &ctx);
                }

                ctx.ui.reload_browser(&action_name);

                // All connections are closed when the `TcpStream` inside those
                // `WebSocket` is dropped.
                sockets.lock().unwrap().clear();
            }
        });
    }

    // Listen for new WS connections, accept them and push them in the vector.
    let server = TcpListener::bind(config.ws_addr())?;
    ctx.ui.listening_ws(&config.ws_addr());
    init_done.send(()).unwrap();

    for stream in server.incoming() {
        let websocket = tungstenite::accept(stream?)?;
        sockets.lock().unwrap().push(websocket);
    }

    Ok(())
}

fn wait_until_socket_open(target: SocketAddr, ctx: &Context) {
    const POLL_PERIOD: Duration = Duration::from_millis(20);
    const PORT_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

    let start_wait = Instant::now();

    while start_wait.elapsed() < PORT_WAIT_TIMEOUT {
        let before_connect = Instant::now();
        if TcpStream::connect_timeout(&target, POLL_PERIOD).is_ok() {
            return;
        }

        if let Some(remaining) = POLL_PERIOD.checked_sub(before_connect.elapsed()) {
            thread::sleep(remaining);
        }
    }

    ctx.ui.port_wait_timeout(target, PORT_WAIT_TIMEOUT);
}
