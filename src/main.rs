mod config;
mod tokiort;
mod clone;
mod http_utils;
mod proxy;

use hyper::service::service_fn;

use hyper::{Request, Response, server};
use http_body_util::{combinators::BoxBody};

use bytes::Bytes;
use tokio::net::{TcpListener, TcpStream};

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use crate::tokiort::TokioIo;

use hyper::Method;
use hyper::upgrade::Upgraded;

fn host_addr(uri: &http::Uri) -> Option<String> {
    uri.authority().and_then(|auth| Some(auth.to_string()))
}

// Create a TCP connection to host:port, build a tunnel between the connection and
// the upgraded connection
async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    // Connect to remote server
    let mut server = TcpStream::connect(addr).await?;
    let mut upgraded = TokioIo::new(upgraded);

    // Proxying data
    let (_from_client, _from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    Ok(())
}

async fn proxy_handler(
   config : &config::Config,
   req: Request<hyper::body::Incoming>,
)
   -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let method = req.method().clone();
    let path = req.uri().path();
    println!("request  for     {} {}", &method, &path);

    if Method::CONNECT == req.method() {
        // Received an HTTP request like:
        // ```
        // CONNECT www.domain.com:443 HTTP/1.1
        // Host: www.domain.com:443
        // Proxy-Connection: Keep-Alive
        // ```
        //
        // When HTTP method is CONNECT we should return an empty body
        // then we can eventually upgrade the connection and talk a new protocol.
        //
        // Note: only after client received an empty body with STATUS_OK can the
        // connection be upgraded, so we can't return a response inside
        // `on_upgrade` future.
        if let Some(addr) = host_addr(req.uri()) {
            tokio::task::spawn(async move {
                match hyper::upgrade::on(req).await {
                    Ok(upgraded) => {
                        if let Err(e) = tunnel(upgraded, addr).await {
                            eprintln!("server io error: {}", e);
                        };
                    }
                    Err(e) => eprintln!("upgrade error: {}", e),
                }
            });

            Ok(Response::new(http_utils::empty()))
        } else {
            eprintln!("CONNECT host is not socket addr: {:?}", req.uri());
            let mut resp = Response::new(http_utils::full("CONNECT must be to a socket address"));
            *resp.status_mut() = http::StatusCode::BAD_REQUEST;

            Ok(resp)
        }
    } else {
        if config.replay_only {
            proxy::replay(config, req).await
        } else {
            if proxy::recording_exists(&proxy::recording_name(&config.tapes, &req)) {
                return proxy::replay(config, req).await;
            }

            let (req, new_req) = clone::clone_incoming_request(req).await?;
            let resp = proxy::make_request(new_req).await?;
            let (resp, new_resp) = clone::clone_incoming_response(resp).await?;
            let _ = proxy::record(&config, req, new_resp).await;
            Ok(resp)
        }
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::get_config().await;

    let ip =
        IpAddr::from_str(&config.bind).expect("Looks like you didn't provide a valid IP for bind");
    let addr = SocketAddr::new(ip, config.port);
    println!("Listening on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let config = config.clone();
        tokio::task::spawn(async move {
            if let Err(err) = server::conn::http1::Builder::new()
                .serve_connection(io, service_fn(|req| async {
                   proxy_handler(&config, req).await
                }))
                .with_upgrades()
                .await
            {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}
