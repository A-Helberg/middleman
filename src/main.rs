mod clone;
mod config;
mod http_utils;
mod proxy;
mod tokiort;

use hyper::service::service_fn;

use http_body_util::combinators::BoxBody;
use hyper::{server, Request, Response};

use bytes::Bytes;
use std::fs::File;
use std::io::{self, BufReader};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{rustls, TlsAcceptor};

use crate::tokiort::TokioIo;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use crate::clone::clone_incoming_response;
use crate::config::Config;

use hyper::upgrade::Upgraded;
use hyper::Method;

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
    config: &config::Config,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
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
            let passthrough = req.headers().contains_key("x-middleman-passthrough")
                && req.headers().get("x-middleman-passthrough").unwrap() != "false";

            if passthrough == true {
                let (req, _) = clone::clone_incoming_request(req).await?;
                let resp = proxy::make_request(config, req).await?;
                let (_, resp) = clone_incoming_response(resp).await?;
                return Ok(resp);
            }

            if proxy::recording_exists(&proxy::recording_name(&config.tapes, &req)) {
                return proxy::replay(config, req).await;
            }

            let (req, new_req) = clone::clone_incoming_request(req).await?;
            let resp = proxy::make_request(config, new_req).await?;
            let (resp, new_resp) = clone::clone_incoming_response(resp).await?;
            let _ = proxy::record(&config, req, new_resp).await;
            Ok(resp)
        }
    }
}

async fn listen_and_serve_https(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let ip =
        IpAddr::from_str(&config.bind).expect("Looks like you didn't provide a valid IP for bind");
    let tls_addr = SocketAddr::new(ip, config.tls_port);

    if config.listen_tls {
        println!("TLS Listening on {}", tls_addr);
        let tls_listener = TcpListener::bind(&tls_addr).await?;

        let cert_file_path = config.cert_file.clone().unwrap();
        let private_key_file_path = config.private_key_file.clone().unwrap();

        let cert_file = File::open(cert_file_path.clone());
        if cert_file.is_err() {
            panic!("Could not open cert file: {cert_file_path}")
        }

        let certs = rustls_pemfile::certs(&mut BufReader::new(cert_file.unwrap()))
            .collect::<Result<Vec<_>, _>>()?;

        let private_key_file = File::open(private_key_file_path.clone());
        if private_key_file.is_err() {
            panic!("Could not open private key file: {private_key_file_path}")
        }

        let private_key =
            rustls_pemfile::private_key(&mut BufReader::new(private_key_file.unwrap()))?.unwrap();

        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, private_key)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        let acceptor = TlsAcceptor::from(Arc::new(server_config));
        loop {
            let (stream, _) = tls_listener.accept().await?;

            let config = config.clone();
            let acceptor = acceptor.clone();

            let stream = acceptor.accept(stream).await;
            if stream.is_err() {
                println!("Err: HTTP connection to HTTPS server");
                continue;
            }

            let stream = stream.unwrap();

            let io = TokioIo::new(stream);
            tokio::task::spawn(async move {
                if let Err(err) = server::conn::http1::Builder::new()
                    .serve_connection(
                        io,
                        service_fn(|req| async { proxy_handler(&config, req).await }),
                    )
                    .with_upgrades()
                    .await
                {
                    println!("Failed to serve connection: {:?}", err);
                }
            });
        }
    }
    Ok(())
}

async fn listen_and_serve_http(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let ip =
        IpAddr::from_str(&config.bind).expect("Looks like you didn't provide a valid IP for bind");
    let addr = SocketAddr::new(ip, config.port);

    println!("Listening on {} for HTTP", addr);

    let listener = TcpListener::bind(&addr).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let config = config.clone();
        tokio::task::spawn(async move {
            if let Err(err) = server::conn::http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(|req| async { proxy_handler(&config, req).await }),
                )
                .with_upgrades()
                .await
            {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::get_config().await;

    let (a, b) = tokio::join!(
        listen_and_serve_http(&config),
        listen_and_serve_https(&config)
    );
    a?;
    b?;
    Ok(())
}
