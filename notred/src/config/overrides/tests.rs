use std::path::Path;

use super::load_events_policy;
use crate::config::FileConfig;

fn examples_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples"))
}

#[test]
fn load_examples_override_tree() {
    let config_path = examples_dir().join("config.toml");
    let file = FileConfig::load(Some(&config_path)).unwrap();
    let dir = config_path.parent().unwrap();
    let policy = load_events_policy(&file.events, &file.paths.overrides, dir).unwrap();
    assert_eq!(policy.overrides.len(), 3);

    let app = policy
        .overrides
        .iter()
        .find(|ov| matches!(&ov.kind, libnotred::OverrideKind::App { name } if name == "some_app"))
        .expect("some_app fragment");
    assert_eq!(app.nested.len(), 2);
}

#[test]
fn merge_per_app_button_left() {
    let config_path = examples_dir().join("config.toml");
    let mut file = FileConfig::load(Some(&config_path)).unwrap();
    file.events.on_button_left = Some(vec!["echo".into(), "root".into()]);
    let dir = config_path.parent().unwrap();
    let policy = load_events_policy(&file.events, &file.paths.overrides, dir).unwrap();

    let app_hooks = libnotred::EventsHooks {
        on_button_left: Some(vec!["echo".into(), "app".into()]),
        ..Default::default()
    };

    let app_idx = policy
        .overrides
        .iter()
        .position(
            |ov| matches!(&ov.kind, libnotred::OverrideKind::App { name } if name == "some_app"),
        )
        .unwrap();
    // Inject hook for test (examples file has it commented out)
    let mut policy = policy;
    policy.overrides[app_idx].hooks = app_hooks;

    let resolved = policy.resolve("some_app", libnotred::wire::Urgency::Normal);
    assert_eq!(
        resolved.on_button_left,
        Some(vec!["echo".into(), "app".into()])
    );

    let other = policy.resolve("other_app", libnotred::wire::Urgency::Normal);
    assert_eq!(
        other.on_button_left,
        Some(vec!["echo".into(), "root".into()])
    );
}
