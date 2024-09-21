use crate::config::Config;
use crate::tokiort::TokioIo;
use crate::{clone, config, http_utils};
use bytes::Bytes;
use http::{Request, Response};
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt;
use hyper::body;
use hyper::body::Incoming;
use hyper::client::conn::http1::Builder;
use native_tls::TlsConnector as NativeTlsConnector;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_native_tls::TlsConnector;

pub fn recording_exists(recording_name: &str) -> bool {
    Path::new(&recording_name).exists()
}

pub fn recording_name<T>(folder: &str, req: &Request<T>) -> String {
    let path: &str = req.uri().path();
    let method: &str = req.method().as_str();
    format!("{}/{}/{}", folder, path, method)
}

pub async fn replay(
    config: &config::Config,
    req: Request<body::Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    if !recording_exists(&recording_name(&config.tapes, &req)) {
        println!(
            "Not Impl for {} {} {}",
            501,
            &req.method().to_string(),
            &req.uri().path().to_string()
        );

        let mut resp = Response::builder().status(501);

        if req.headers().get("accept").is_some() {
            resp = resp.header("accept", req.headers().get("accept").unwrap());
        }
        return Ok(resp.body(http_utils::empty()).unwrap());
    }
    let recording_path = recording_name(&config.tapes, &req);
    let c: Vec<u8> = fs::read(&recording_path).await.unwrap();
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut resp = httparse::Response::new(&mut headers);

    resp.parse(&c).unwrap();

    let mut response_builder = Response::builder().status(resp.code.unwrap());

    let method = req.method().clone();
    let path = req.uri().path();
    println!("playback for {} {} {}", resp.code.unwrap(), &method, &path);

    for header in resp.headers {
        response_builder = response_builder.header(header.name, header.value);
    }

    let start_of_body = http_utils::start_of_body(&c);
    let body: Bytes = c[start_of_body..].to_vec().into();
    //<&[u8] as TryInto<Bytes>>::try_into(c[start_of_body..]).unwrap().clone();
    let response = response_builder.body(http_utils::full(body)).unwrap();

    Ok(response)
}

pub async fn record(
    config: &config::Config,
    req: Request<BoxBody<Bytes, hyper::Error>>,
    resp: Response<BoxBody<Bytes, hyper::Error>>,
) -> Result<(), hyper::Error> {
    let method = req.method().clone();
    let path = req.uri().path();

    println!(
        "record   for {} {} {}",
        &resp.status().as_u16(),
        &method,
        &path
    );
    let recording_path = recording_name(&config.tapes, &req);
    fs::create_dir_all(format!("{}/{}", &config.tapes, &path))
        .await
        .expect("Failed to create a tape directory");

    let mut file = tokio::fs::File::create(&recording_path)
        .await
        .expect("Could not write to the tapes directory");

    let (resp, new_resp) = clone::clone_bytes_response(resp).await?;
    let (_parts, body) = new_resp.into_parts();

    let preamble = format!(
        "{:?} {} {}\r\n",
        &resp.version(),
        &resp.status().as_str(),
        &resp.status().canonical_reason().or(Some("")).unwrap()
    );

    let _ = file.write_all(preamble.as_bytes()).await;
    for (name, value) in resp.headers() {
        let header = format!("{}:{}\r\n", name, value.to_str().unwrap());
        let _ = file.write_all(header.as_bytes()).await;
    }
    let _ = file.write_all("\r\n".as_bytes()).await;

    let x = body.collect().await?.aggregate();
    let body = clone::clone_body(x);

    let _ = file.write_all(&body).await;

    Ok(())
}

pub async fn make_request_insecure(
    config: &Config,
    req: Request<BoxBody<Bytes, hyper::Error>>,
) -> Result<Response<Incoming>, hyper::Error> {
    let host = config.upstream_ip.clone();
    let port = config.upstream_port;

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

    sender.send_request(req).await
}

pub async fn make_request_secure(
    config: &Config,
    req: Request<BoxBody<Bytes, hyper::Error>>,
) -> Result<Response<Incoming>, hyper::Error> {
    let ip = config.upstream_ip.clone();
    let port = config.upstream_port;

    let stream = TcpStream::connect((ip, port)).await.unwrap();

    let native_connector = NativeTlsConnector::builder().build().unwrap();
    let stream = TlsConnector::from(native_connector)
        .connect(&config.upstream, stream)
        .await
        .unwrap();

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

    sender.send_request(req).await
}

pub async fn make_request(
    config: &Config,
    req: Request<BoxBody<Bytes, hyper::Error>>,
) -> Result<Response<Incoming>, hyper::Error> {
    if config.upstream_tls {
        return make_request_secure(config, req).await;
    } else {
        return make_request_insecure(config, req).await;
    }
}
