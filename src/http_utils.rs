use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};

pub fn start_of_body(payload: &Vec<u8>) -> usize {
    let mut start_of_body = 0;
    for i in 0..payload.len() {
        let bs = "\r\n".as_bytes();
        if i + 3 < payload.len() {
            if payload[i] == bs[0]
                && payload[i + 1] == bs[1]
                && payload[i + 2] == bs[0]
                && payload[i + 3] == bs[1]
            {
                start_of_body = i + 4;
                break;
            }
        }
    }
    start_of_body
}

pub fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}
