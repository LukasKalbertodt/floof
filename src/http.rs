use anyhow::{bail, Result};
// use futures_util::TryStreamExt;
use hyper::{
    Body, Client, Request, Response, Server, Uri, StatusCode,
    service::{make_service_fn, service_fn}
};

use crate::{
    config,
    ui::Ui,
};
use std::sync::Arc;


#[tokio::main]
pub async fn run(config: &config::Http, ui: Ui) -> Result<()> {
    let addr = config.addr();

    let service = if let Some(proxy_target) = &config.proxy {
        let target = Arc::new(proxy_target.clone());

        make_service_fn(move |_| {
            let target = target.clone();
            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    proxy(req, target.clone())
                }))
            }
        })
    } else {
        bail!("bug: invalid http config");
    };

    let server = Server::bind(&addr).serve(service);
    ui.listening(&addr);

    server.await?;

    Ok(())
}


async fn proxy(mut req: Request<Body>, target: Arc<String>) -> Result<Response<Body>, hyper::Error> {
    let uri = Uri::builder()
        .scheme("http")
        .authority(target.as_str())
        .path_and_query(req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or(""))
        .build()
        .expect("failed to build URI");
    *req.uri_mut() = uri.clone();

    let client = Client::new();
    let out = client.request(req).await.unwrap_or_else(|e| {
        let msg = format!("failed to reach {}\nError:\n\n{}", uri, e);

        Response::builder()
            // TODO: sometimes this should be 504 GATEWAY TIMEOUT
            .status(StatusCode::BAD_GATEWAY)
            .header("Content-Type", "text/plain")
            .body(msg.into())
            .unwrap()
    });

    Ok(out)
}
