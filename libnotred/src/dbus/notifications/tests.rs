use super::{icon_from_hints, icon_from_image_data, icon_from_str, urgency_from_hints};
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
