use crate::config::events::EventsConfig;

#[test]
fn all_hooks_deserialize() {
    let toml = r#"
on_action = ["echo", "action"]
on_button_left = ["echo", "left"]
on_notify = ["echo", "notify"]
"#;
    let cfg: EventsConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.on_action, Some(vec!["echo".into(), "action".into()]));
    assert_eq!(cfg.on_button_left, Some(vec!["echo".into(), "left".into()]));
    assert_eq!(cfg.on_notify, Some(vec!["echo".into(), "notify".into()]));
    assert!(cfg.on_touch.is_none());
}
