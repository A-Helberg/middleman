use bytes::Bytes;
use hyper::{Request, Response};
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt;
use hyper::body::{Incoming, Buf};
use hyper;
use crate::http_utils::full;


macro_rules! two_of {
    // The macro will match against any type `T`
    ($type:ty) => {
        // And it will produce a tuple of two of that type
        ($type, $type)
    };
}
pub fn clone_body<T : Buf>(mut y : T) -> Vec<u8> {
    let mut body : Vec<u8>= Vec::with_capacity(1000);
    while y.has_remaining() {
        if body.capacity() == body.len() {
            body.reserve(100);
        }
        body.push(y.get_u8());
    }
    body
}

type MyResponse = Result<two_of!(Response<BoxBody<Bytes, hyper::Error>>), hyper::Error>;
type MyRequest = Result<two_of!(Request<BoxBody<Bytes, hyper::Error>>), hyper::Error>;


pub async fn clone_incoming_request(req: Request<Incoming>) -> MyRequest {
    let (parts, body) = req.into_parts();
    //req.collect().await?.aggregate();
    let x = body.collect().await?.aggregate();
    let body = clone_body(x);

    Ok((
        Request::from_parts(parts.clone(), full(body.clone())),
       Request::from_parts(parts.clone(), full(body.clone()))
    ))
}

#[allow(dead_code)]
pub async fn clone_bytes_request(req: Request<BoxBody<Bytes, hyper::Error>>) -> MyRequest {
    let (parts, body) = req.into_parts();
    //req.collect().await?.aggregate();
    let x = body.collect().await?.aggregate();
    let body = clone_body(x);

    Ok((Request::from_parts(parts.clone(), full(body.clone())), Request::from_parts(parts, full(body))))
}
// Response
pub async fn clone_incoming_response(req: Response<Incoming>) -> MyResponse {
    let (parts, body) = req.into_parts();
    //req.collect().await?.aggregate();
    let x = body.collect().await?.aggregate();
    let body = clone_body(x);

    Ok((
        Response::from_parts(parts.clone(), full(body.clone())),
        Response::from_parts(parts.clone(), full(body.clone()))
    ))
}

pub async fn clone_bytes_response(req: Response<BoxBody<Bytes, hyper::Error>>) -> MyResponse {
    let (parts, body) = req.into_parts();
    //req.collect().await?.aggregate();
    let x = body.collect().await?.aggregate();
    let body = clone_body(x);

    Ok((Response::from_parts(parts.clone(), full(body.clone())),
        Response::from_parts(parts, full(body))))
}
