use tokio::io::{BufReader, duplex};

use crate::ipc::codec;
use crate::wire::{Cmd, OkPayload, Request, Response};

#[tokio::test]
async fn write_request_read_request_roundtrip() {
    let (mut a, b) = duplex(256);
    let req = Request::new(Cmd::Ping);
    codec::write_request(&mut a, &req).await.unwrap();
    drop(a);

    let mut reader = BufReader::new(b);
    let got = codec::read_request(&mut reader).await.unwrap().unwrap();
    assert_eq!(got, req);
}

#[tokio::test]
async fn write_response_read_response_roundtrip() {
    let (mut a, b) = duplex(256);
    let resp = Response::ok(OkPayload::Pong);
    codec::write_response(&mut a, &resp).await.unwrap();
    drop(a);

    let mut reader = BufReader::new(b);
    let got = codec::read_response(&mut reader).await.unwrap().unwrap();
    assert_eq!(got, resp);
}
