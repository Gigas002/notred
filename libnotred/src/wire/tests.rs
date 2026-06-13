use super::{Cmd, Event, OkPayload, Request, Response, V};

#[test]
fn golden_ping_roundtrip() {
    let req_line = r#"{"v":1,"cmd":"ping"}"#;
    let resp_line = r#"{"v":1,"type":"pong"}"#;

    let req: Request = serde_json::from_str(req_line).unwrap();
    assert_eq!(req.v, V);
    assert_eq!(req.cmd, Cmd::Ping);

    let resp: Response = serde_json::from_str(resp_line).unwrap();
    assert_eq!(resp, Response::ok(OkPayload::Pong));
}

#[test]
fn golden_subscribe_empty_update() {
    let resp_line = r#"{"v":1,"type":"event","event":{"kind":"update","items":[]}}"#;
    let resp: Response = serde_json::from_str(resp_line).unwrap();
    assert_eq!(
        resp,
        Response::ok(OkPayload::Event {
            event: Event::Update { items: vec![] },
        })
    );
}

#[test]
fn golden_list_empty() {
    let req_line = r#"{"v":1,"cmd":"list"}"#;
    let resp_line = r#"{"v":1,"type":"items","items":[]}"#;

    let req: Request = serde_json::from_str(req_line).unwrap();
    assert_eq!(req.cmd, Cmd::List);

    let resp: Response = serde_json::from_str(resp_line).unwrap();
    assert_eq!(resp, Response::ok(OkPayload::Items { items: vec![] }));
}

#[test]
fn request_serialize_ping() {
    let json = serde_json::to_string(&Request::new(Cmd::Ping)).unwrap();
    assert_eq!(json, r#"{"v":1,"cmd":"ping"}"#);
}

#[test]
fn dismiss_roundtrip() {
    let req_line = r#"{"v":1,"cmd":"dismiss","id":42}"#;
    let req: Request = serde_json::from_str(req_line).unwrap();
    assert_eq!(req.cmd, Cmd::Dismiss { id: 42 });

    let json = serde_json::to_string(&Request::new(Cmd::Dismiss { id: 42 })).unwrap();
    assert_eq!(json, req_line);
}

#[test]
fn close_all_roundtrip() {
    let req_line = r#"{"v":1,"cmd":"close_all"}"#;
    let req: Request = serde_json::from_str(req_line).unwrap();
    assert_eq!(req.cmd, Cmd::CloseAll);

    let json = serde_json::to_string(&Request::new(Cmd::CloseAll)).unwrap();
    assert_eq!(json, req_line);
}

#[test]
fn ok_payload_roundtrip() {
    let resp_line = r#"{"v":1,"type":"ok"}"#;
    let resp: Response = serde_json::from_str(resp_line).unwrap();
    assert_eq!(resp, Response::ok(OkPayload::Ok));

    let json = serde_json::to_string(&Response::ok(OkPayload::Ok)).unwrap();
    assert_eq!(json, resp_line);
}
