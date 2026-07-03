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
