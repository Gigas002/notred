use super::FileConfig;

#[test]
fn xdg_path_is_under_notred_config_dir() {
    assert!(FileConfig::xdg_path().ends_with("notred/tui.toml"));
}

#[test]
fn empty_toml_is_default() {
    let cfg: FileConfig = toml::from_str("").unwrap();
    assert!(cfg.ctl.is_none());
    assert!(cfg.socket.is_none());
}

#[test]
fn ctl_and_socket_parse() {
    let cfg: FileConfig = toml::from_str(
        r#"
ctl = "/usr/bin/notredctl"
socket = "/run/notred.sock"
"#,
    )
    .unwrap();
    assert_eq!(
        cfg.ctl.as_deref().unwrap().to_str().unwrap(),
        "/usr/bin/notredctl"
    );
    assert_eq!(
        cfg.socket.as_deref().unwrap().to_str().unwrap(),
        "/run/notred.sock"
    );
}
