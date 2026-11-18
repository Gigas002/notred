use super::{HistoryRow, HistoryState, Urgency};

#[test]
fn deserializes_list_history_array() {
    let json = r#"[
      {
        "id": 1,
        "app_id": "app",
        "summary": "Hi",
        "body": "Body",
        "urgency": "normal",
        "timeout_ms": -1,
        "has_actions": false,
        "action_keys": [],
        "received_at": 100,
        "state": "active"
      }
    ]"#;
    let rows: Vec<HistoryRow> = serde_json::from_str(json).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 1);
    assert_eq!(rows[0].state, HistoryState::Active);
    assert_eq!(rows[0].urgency, Urgency::Normal);
}

#[test]
fn accepts_every_icon_shape() {
    // `notredctl list-history` as sent for a name, a path and an image-data
    // avatar (chat apps); one unknown shape must not hide the whole list.
    let json = r#"[
      {"id": 3, "app_id": "tg", "summary": "avatar", "body": "", "urgency": "normal",
       "timeout_ms": 0, "has_actions": false, "action_keys": [], "received_at": 3,
       "state": "active",
       "icon": {"width": 2, "height": 1, "rowstride": 6, "has_alpha": false,
                "bits_per_sample": 8, "channels": 3, "data": "/wAAAP8A"}},
      {"id": 2, "app_id": "a", "summary": "path", "body": "", "urgency": "low",
       "timeout_ms": 0, "has_actions": false, "action_keys": [], "received_at": 2,
       "state": "closed", "icon": {"path": "/x.png"}},
      {"id": 1, "app_id": "a", "summary": "name", "body": "", "urgency": "critical",
       "timeout_ms": 0, "has_actions": false, "action_keys": [], "received_at": 1,
       "state": "closed", "icon": {"name": "mail"}}
    ]"#;
    let rows: Vec<HistoryRow> = serde_json::from_str(json).unwrap();
    assert_eq!(rows.len(), 3);
}
