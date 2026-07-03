use super::{EventsHooks, EventsPolicy, LoadedEventOverride, OverrideKind};
use crate::wire::Urgency;

fn hook(cmd: &str) -> Vec<String> {
    vec!["echo".into(), cmd.into()]
}

#[test]
fn merge_field_by_field() {
    let mut base = EventsHooks {
        on_button_left: Some(hook("root-left")),
        on_notify: Some(hook("root-notify")),
        ..Default::default()
    };
    let overlay = EventsHooks {
        on_button_left: Some(hook("app-left")),
        ..Default::default()
    };
    base.merge_from(&overlay);
    assert_eq!(base.on_button_left, Some(hook("app-left")));
    assert_eq!(base.on_notify, Some(hook("root-notify")));
}

#[test]
fn resolve_layers_app_and_urgency() {
    let policy = EventsPolicy {
        base: EventsHooks {
            on_button_left: Some(hook("root")),
            ..Default::default()
        },
        overrides: vec![
            LoadedEventOverride {
                kind: OverrideKind::Urgency {
                    level: Urgency::Critical,
                },
                hooks: EventsHooks {
                    on_button_right: Some(hook("global-critical-right")),
                    ..Default::default()
                },
                nested: vec![],
            },
            LoadedEventOverride {
                kind: OverrideKind::App {
                    name: "firefox".into(),
                },
                hooks: EventsHooks {
                    on_button_left: Some(hook("firefox-left")),
                    ..Default::default()
                },
                nested: vec![LoadedEventOverride {
                    kind: OverrideKind::Urgency {
                        level: Urgency::Critical,
                    },
                    hooks: EventsHooks {
                        on_notify: Some(hook("firefox-critical-notify")),
                        ..Default::default()
                    },
                    nested: vec![],
                }],
            },
        ],
    };

    let hooks = policy.resolve("firefox", Urgency::Critical);
    assert_eq!(hooks.on_button_left, Some(hook("firefox-left")));
    assert_eq!(hooks.on_button_right, Some(hook("global-critical-right")));
    assert_eq!(hooks.on_notify, Some(hook("firefox-critical-notify")));

    let normal = policy.resolve("firefox", Urgency::Normal);
    assert_eq!(normal.on_button_left, Some(hook("firefox-left")));
    assert_eq!(normal.on_button_right, None);
    assert_eq!(normal.on_notify, None);
}
