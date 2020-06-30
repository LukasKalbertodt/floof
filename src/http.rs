use anyhow::{bail, Result};
use hyper::{
    Body, Client, Request, Response, Server, Uri, StatusCode,
    header,
    service::{make_service_fn, service_fn}
};
use kuchiki::traits::*;

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
        let auto_reload = config.auto_reload.unwrap_or(true);

        make_service_fn(move |_| {
            let target = target.clone();
            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    proxy(req, target.clone(), auto_reload)
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


async fn proxy(
    mut req: Request<Body>,
    target: Arc<String>,
    auto_reload: bool,
) -> Result<Response<Body>> {
    let uri = Uri::builder()
        .scheme("http")
        .authority(target.as_str())
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
                let s = std::str::from_utf8(&body)?;
                let document = kuchiki::parse_html().one(s);
                let script = format!("<script>{}</script>", include_str!("inject.js"));
                let script = kuchiki::parse_html().one(&*script);

                let head = document.select("head").unwrap().next().unwrap();
                // let () = head;
                head.as_node().append(script);

                let mut new_body_data = Vec::new();
                document.serialize(&mut new_body_data).expect("failed to write into vector");
                let new_len = new_body_data.len();
                let new_body = Body::from(new_body_data);

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
            let msg = format!("failed to reach {}\nError:\n\n{}", uri, e);

            Response::builder()
                // TODO: sometimes this should be 504 GATEWAY TIMEOUT
                .status(StatusCode::BAD_GATEWAY)
                .header("Content-Type", "text/plain")
                .body(msg.into())
                .unwrap()
        }
    };

    Ok(response)
}
