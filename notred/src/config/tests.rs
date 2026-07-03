use super::FileConfig;

#[test]
fn defaults_are_sensible() {
    let cfg = FileConfig::default();
    assert!(cfg.socket_path.file_name().unwrap() == "notred.sock");
    assert!(!cfg.log_filter.is_empty());
}

#[test]
fn load_from_toml_string() {
    let toml = r#"
socket_path = "/tmp/test.sock"
log_filter  = "debug"
"#;
    let cfg: FileConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.socket_path.to_str().unwrap(), "/tmp/test.sock");
    assert_eq!(cfg.log_filter, "debug");
}

#[test]
fn missing_fields_fall_back_to_defaults() {
    let cfg: FileConfig = toml::from_str("").unwrap();
    assert!(cfg.socket_path.ends_with("notred.sock"));
    assert!(!cfg.log_filter.is_empty());
}

#[test]
fn load_returns_defaults_when_no_file() {
    let cfg = FileConfig::load(None).unwrap();
    assert!(cfg.socket_path.ends_with("notred.sock"));
}

#[test]
fn events_on_action_parses() {
    let toml = r#"
[events]
on_action = ["echo", "hi"]
"#;
    let cfg: FileConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.events.on_action, Some(vec!["echo".into(), "hi".into()]));
}

#[test]
fn load_explicit_missing_path_errors() {
    let result = FileConfig::load(Some(std::path::Path::new("/nonexistent/path/notred.toml")));
    assert!(result.is_err());
}

#[cfg(feature = "history")]
#[test]
fn history_defaults() {
    let cfg = FileConfig::default();
    assert!(!cfg.history.enabled);
    assert!(cfg.history.flush);
    assert_eq!(cfg.history.max_entries, 5);
    assert!(cfg.history_path.ends_with("history.db"));
}

#[cfg(feature = "history")]
#[test]
fn history_section_parses() {
    let toml = r#"
[history]
enabled = false
flush = false
max_entries = 0
"#;
    let cfg: FileConfig = toml::from_str(toml).unwrap();
    assert!(!cfg.history.enabled);
    assert!(!cfg.history.flush);
    assert_eq!(cfg.history.max_entries, 0);
}
