use super::{
    Notifications, icon_from_hints, icon_from_image_data, icon_from_str, string_hint,
    urgency_from_hints, value_from_hints,
};
use crate::host::state::{HostState, RuntimeConfig};
use crate::queue::Queue;
use crate::wire::{IconRef, Urgency};
use std::collections::HashMap;
use std::sync::Arc;

fn notifications_with(runtime: RuntimeConfig) -> Notifications {
    let state = HostState::new(runtime, Arc::new(Queue::new()));
    Notifications::new(state)
}

#[test]
fn icon_empty_is_none() {
    assert!(icon_from_str("").is_none());
}

#[test]
fn icon_name_without_slash() {
    assert!(matches!(
        icon_from_str("dialog-information"),
        Some(IconRef::Name { .. })
    ));
}

#[test]
fn icon_path_with_slash() {
    assert!(matches!(
        icon_from_str("/usr/share/icons/foo.png"),
        Some(IconRef::Path { .. })
    ));
}

fn image_data_hint(
    width: i32,
    height: i32,
    has_alpha: bool,
    data: Vec<u8>,
) -> HashMap<String, zbus::zvariant::OwnedValue> {
    use zbus::zvariant::{Structure, Value};

    let channels = if has_alpha { 4 } else { 3 };
    let rowstride = width * channels;
    let structure: Structure = (width, height, rowstride, has_alpha, 8i32, channels, data).into();

    let mut hints = HashMap::new();
    hints.insert(
        "image-data".into(),
        Value::Structure(structure).try_into().unwrap(),
    );
    hints
}

#[test]
fn icon_from_image_data_parses_raw_pixels() {
    let hints = image_data_hint(2, 1, true, vec![255, 0, 0, 255, 0, 255, 0, 128]);
    assert_eq!(
        icon_from_image_data(&hints),
        Some(IconRef::Raw {
            width: 2,
            height: 1,
            rowstride: 8,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![255, 0, 0, 255, 0, 255, 0, 128],
        })
    );
}

#[test]
fn icon_from_image_data_none_without_hint() {
    let hints = HashMap::new();
    assert!(icon_from_image_data(&hints).is_none());
}

#[test]
fn icon_from_image_data_none_for_empty_pixels() {
    let hints = image_data_hint(0, 0, false, vec![]);
    assert!(icon_from_image_data(&hints).is_none());
}

#[test]
fn icon_from_hints_reads_image_path() {
    use zbus::zvariant::Value;

    let mut hints = HashMap::new();
    hints.insert(
        "image-path".into(),
        Value::Str("web-browser".into()).try_into().unwrap(),
    );
    assert_eq!(
        icon_from_hints(&hints),
        Some(IconRef::Name {
            name: "web-browser".into()
        })
    );
}

#[test]
fn icon_from_hints_strips_file_uri_prefix() {
    use zbus::zvariant::Value;

    let mut hints = HashMap::new();
    hints.insert(
        "image-path".into(),
        Value::Str("file:///tmp/icon.png".into())
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        icon_from_hints(&hints),
        Some(IconRef::Path {
            path: "/tmp/icon.png".into()
        })
    );
}

#[test]
fn icon_from_hints_falls_back_to_legacy_underscore_key() {
    use zbus::zvariant::Value;

    let mut hints = HashMap::new();
    hints.insert(
        "image_path".into(),
        Value::Str("firefox".into()).try_into().unwrap(),
    );
    assert_eq!(
        icon_from_hints(&hints),
        Some(IconRef::Name {
            name: "firefox".into()
        })
    );
}

#[test]
fn icon_from_hints_none_without_image_path() {
    let hints = HashMap::new();
    assert_eq!(icon_from_hints(&hints), None);
}

#[test]
fn urgency_default_is_normal() {
    let hints = HashMap::new();
    assert_eq!(urgency_from_hints(&hints), Urgency::Normal);
}

#[test]
fn urgency_low_critical_parsed() {
    use zbus::zvariant::{OwnedValue, Value};

    let low: OwnedValue = Value::U8(0).try_into().unwrap();
    let critical: OwnedValue = Value::U8(2).try_into().unwrap();

    let mut hints = HashMap::new();
    hints.insert("urgency".into(), low);
    assert_eq!(urgency_from_hints(&hints), Urgency::Low);

    let mut hints2 = HashMap::new();
    hints2.insert("urgency".into(), critical);
    assert_eq!(urgency_from_hints(&hints2), Urgency::Critical);
}

#[test]
fn value_absent_is_none() {
    let hints = HashMap::new();
    assert_eq!(value_from_hints(&hints), None);
}

#[test]
fn value_in_range_is_parsed() {
    use zbus::zvariant::Value;

    let mut hints = HashMap::new();
    hints.insert("value".into(), Value::I32(42).try_into().unwrap());
    assert_eq!(value_from_hints(&hints), Some(42));
}

#[test]
fn value_out_of_range_is_none() {
    use zbus::zvariant::Value;

    let mut over = HashMap::new();
    over.insert("value".into(), Value::I32(101).try_into().unwrap());
    assert_eq!(value_from_hints(&over), None);

    let mut under = HashMap::new();
    under.insert("value".into(), Value::I32(-1).try_into().unwrap());
    assert_eq!(value_from_hints(&under), None);
}

#[test]
fn value_boundary_values_are_parsed() {
    use zbus::zvariant::Value;

    let mut zero = HashMap::new();
    zero.insert("value".into(), Value::I32(0).try_into().unwrap());
    assert_eq!(value_from_hints(&zero), Some(0));

    let mut hundred = HashMap::new();
    hundred.insert("value".into(), Value::I32(100).try_into().unwrap());
    assert_eq!(value_from_hints(&hundred), Some(100));
}

#[test]
fn string_hint_reads_category() {
    use zbus::zvariant::Value;

    let mut hints = HashMap::new();
    hints.insert(
        "category".into(),
        Value::Str("email.arrived".into()).try_into().unwrap(),
    );
    assert_eq!(
        string_hint(&hints, "category"),
        Some("email.arrived".into())
    );
}

#[test]
fn string_hint_reads_desktop_entry() {
    use zbus::zvariant::Value;

    let mut hints = HashMap::new();
    hints.insert(
        "desktop-entry".into(),
        Value::Str("firefox".into()).try_into().unwrap(),
    );
    assert_eq!(string_hint(&hints, "desktop-entry"), Some("firefox".into()));
}

#[test]
fn string_hint_absent_is_none() {
    let hints = HashMap::new();
    assert_eq!(string_hint(&hints, "category"), None);
}

#[tokio::test]
async fn capabilities_include_body_markup_when_enabled() {
    let n = notifications_with(RuntimeConfig {
        body_markup: true,
        ..RuntimeConfig::default()
    });
    assert!(
        n.get_capabilities()
            .await
            .contains(&"body-markup".to_string())
    );
}

#[tokio::test]
async fn capabilities_omit_body_markup_when_disabled() {
    let n = notifications_with(RuntimeConfig {
        body_markup: false,
        ..RuntimeConfig::default()
    });
    assert!(
        !n.get_capabilities()
            .await
            .contains(&"body-markup".to_string())
    );
}

#[tokio::test]
async fn notify_tags_notification_with_current_body_markup_setting() {
    let n = notifications_with(RuntimeConfig {
        body_markup: false,
        ..RuntimeConfig::default()
    });
    let id = n
        .notify(
            "app".into(),
            0,
            String::new(),
            "summary".into(),
            "body".into(),
            vec![],
            HashMap::new(),
            -1,
        )
        .await;
    let notif = n.state.queue.get(id).await.unwrap();
    assert!(!notif.body_markup);
    assert_eq!(notif.value, None);
    assert_eq!(notif.category, None);
    assert_eq!(notif.desktop_entry, None);
}

#[tokio::test]
async fn notify_captures_value_category_desktop_entry_hints() {
    use zbus::zvariant::Value;

    let n = notifications_with(RuntimeConfig::default());
    let mut hints = HashMap::new();
    hints.insert("value".into(), Value::I32(55).try_into().unwrap());
    hints.insert(
        "category".into(),
        Value::Str("email.arrived".into()).try_into().unwrap(),
    );
    hints.insert(
        "desktop-entry".into(),
        Value::Str("thunderbird".into()).try_into().unwrap(),
    );

    let id = n
        .notify(
            "app".into(),
            0,
            String::new(),
            "summary".into(),
            "body".into(),
            vec![],
            hints,
            -1,
        )
        .await;
    let notif = n.state.queue.get(id).await.unwrap();
    assert_eq!(notif.value, Some(55));
    assert_eq!(notif.category.as_deref(), Some("email.arrived"));
    assert_eq!(notif.desktop_entry.as_deref(), Some("thunderbird"));
}
