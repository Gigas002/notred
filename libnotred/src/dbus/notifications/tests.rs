use super::{icon_from_str, urgency_from_hints};
use crate::wire::{IconRef, Urgency};
use std::collections::HashMap;

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
