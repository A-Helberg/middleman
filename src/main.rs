mod config;
mod tokiort;
mod clone;

use hyper::service::{ service_fn};

use hyper::{client, Request, Response, server, body};
use hyper::body::{Body, Incoming};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use bytes::Buf;

use bytes::Bytes;
use bytes::BytesMut;

use tokio::net::{TcpListener, TcpStream};

use hyper::header::{HeaderValue};
use hyper_rustls::ConfigBuilderExt;


use std::convert::Infallible;
use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::str::FromStr;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use crate::tokiort::TokioIo;


// TEMP
use hyper::client::conn::http1::Builder;
use hyper::Method;
use hyper::upgrade::Upgraded;
use serde::forward_to_deserialize_any;


fn recording_exists(recording_name: &str) -> bool {
    Path::new(&recording_name).exists()
}

fn recording_name(folder: &str, path: &str, method: &str) -> String {
    format!("{}/{}/{}", folder, path, method)
}

async fn replay(config: &config::Config, req: Request<body::Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let recording_path = recording_name(&config.tapes, &req.uri().to_string(), &req.method().to_string());
    let c: Vec<u8> = fs::read(&recording_path).await.unwrap();
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut resp = httparse::Response::new(&mut headers);

    resp.parse(&c).unwrap();

    let mut response_builder = Response::builder()
        .status(resp.code.unwrap());

    for header in resp.headers {
       response_builder = response_builder.header(header.name, header.value);
    }
    let response = response_builder.body(full(c)).unwrap();

    Ok(response)
}

async fn record(config : &config::Config, req : Request<BoxBody<Bytes,hyper::Error>>, resp: Response<BoxBody<Bytes,hyper::Error>>) -> Result<(),hyper::Error> {
    let recording_path = recording_name(&config.tapes, &req.uri().to_string(), &req.method().to_string());
    fs::create_dir_all(format!("{}/{}", &config.tapes, &req.uri().to_string()))
        .await
        .expect("Failed to create a tape directory");

    let mut file = tokio::fs::File::create(&recording_path)
       .await
       .expect("Could not write to the tapes directory");

    let (resp, new_resp) = clone::clone_bytes_response(resp).await?;
    let (parts, body) = new_resp.into_parts();

    let preamble = format!(
        "{:?} {} {}\r\n",
        &resp.version(),
        &resp.status().as_str(),
        &resp
            .status()
            .canonical_reason()
            .or(Some(""))
            .unwrap()
    );

    let _ = file.write_all(preamble.as_bytes()).await;
    for (name, value) in resp.headers() {
        let header = format!("{}:{}\r\n", name, value.to_str().unwrap());
        let _ = file.write_all(header.as_bytes()).await;
    }
    let _ = file.write_all("\r\n".as_bytes()).await;

    let x = body.collect().await?.aggregate();
    let body = clone::clone_body(x);

    println!("{:?}", body);
    let _ = file.write_all(&body).await;

    Ok(())
}

fn host_addr(uri: &http::Uri) -> Option<String> {
    uri.authority().and_then(|auth| Some(auth.to_string()))
}

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}
fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

// Create a TCP connection to host:port, build a tunnel between the connection and
// the upgraded connection
async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    // Connect to remote server
    let mut server = TcpStream::connect(addr).await?;
    let mut upgraded = TokioIo::new(upgraded);

    // Proxying data
    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    // Print message when done
    println!(
        "client wrote {} bytes and received {} bytes",
        from_client, from_server
    );

    Ok(())
}

async fn proxy_handler(
   config : &config::Config,
   req: Request<hyper::body::Incoming>,
)
   -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {

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

           Ok(Response::new(empty()))
       } else {
           eprintln!("CONNECT host is not socket addr: {:?}", req.uri());
           let mut resp = Response::new(full("CONNECT must be to a socket address"));
           *resp.status_mut() = http::StatusCode::BAD_REQUEST;

           Ok(resp)
       }
   } else {
       if config.replay_only {
           replay(config, req).await
       } else {
           if recording_exists(&recording_name(&config.tapes, &req.uri().to_string(), &req.method().to_string())) {
               return replay(config, req).await;
           }

           let (req, new_req) = clone::clone_incoming_request(req).await?;
           let resp = make_request(new_req).await?;
           let (resp, new_resp) = clone::clone_incoming_response(resp).await?;
           println!("before record {:?}", resp);
           record(&config, req, new_resp).await;
           Ok(resp)
       }
   }
}

async fn make_request(req : Request<BoxBody<Bytes, hyper::Error>>) -> Result<Response<Incoming>, hyper::Error> {

    let host = req.uri().host().expect("uri has no host");
    let port = req.uri().port_u16().unwrap_or(80);

    let stream = TcpStream::connect((host, port)).await.unwrap();
    let io = TokioIo::new(stream);

    let (mut sender, conn) = Builder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .handshake(io)
        .await?;
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });

    //let (req, new_req) = clone::clone_bytes_request(req).await?;

    sender.send_request(req).await
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
