use super::{CloseReason, Notification};
use crate::wire::Urgency;

fn sample() -> Notification {
    Notification {
        id: 1,
        replaces_id: 0,
        app_id: "org.example.App".into(),
        summary: "Title".into(),
        body: "Body".into(),
        urgency: Urgency::Normal,
        timeout_ms: 5000,
        icon: None,
        action_keys: vec![],
        has_actions: false,
        timestamp: 1_000_000,
        value: Some(42),
        category: Some("email.arrived".into()),
        desktop_entry: Some("thunderbird".into()),
        body_markup: true,
    }
}

#[test]
fn to_minimal_maps_fields() {
    let n = sample();
    let m = n.to_minimal();
    assert_eq!(m.id, 1);
    assert_eq!(m.app_id, "org.example.App");
    assert_eq!(m.summary, "Title");
    assert_eq!(m.body, "Body");
    assert_eq!(m.timeout_ms, 5000);
    assert_eq!(m.timestamp, Some(1_000_000));
    assert!(!m.has_actions);
    assert_eq!(m.value, Some(42));
    assert_eq!(m.category.as_deref(), Some("email.arrived"));
    assert_eq!(m.desktop_entry.as_deref(), Some("thunderbird"));
    assert!(m.body_markup);
}

#[test]
fn close_reason_u32() {
    assert_eq!(u32::from(CloseReason::Expired), 1);
    assert_eq!(u32::from(CloseReason::DismissedByUser), 2);
    assert_eq!(u32::from(CloseReason::ClosedByCall), 3);
    assert_eq!(u32::from(CloseReason::Undefined), 4);
}
