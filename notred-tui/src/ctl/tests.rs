use super::SubscribeEvent;

#[test]
fn subscribe_line_parses_history_changed() {
    let json = r#"{"v":1,"type":"event","event":{"kind":"history_changed"}}"#;
    let line: super::SubscribeLine = serde_json::from_str(json).unwrap();
    assert!(line.needs_refresh());
}

#[test]
fn subscribe_line_ignores_pong() {
    let json = r#"{"v":1,"type":"pong"}"#;
    let line: super::SubscribeLine = serde_json::from_str(json).unwrap();
    assert!(!line.needs_refresh());
}

#[test]
fn subscribe_event_enum_values() {
    assert_eq!(SubscribeEvent::Refresh, SubscribeEvent::Refresh);
}
