use super::resolve;
use crate::cli::Command;

#[test]
fn resolve_default_socket_under_runtime_dir() {
    // SAFETY: single-threaded test; no concurrent env access.
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/tmp") };
    let settings = resolve(None, Command::Ping).unwrap();
    assert_eq!(
        settings.socket_path,
        std::path::Path::new("/tmp/notred.sock")
    );
}

#[test]
fn resolve_honors_socket_override() {
    let settings = resolve(Some("/run/notred.sock".into()), Command::Ping).unwrap();
    assert_eq!(settings.socket_path.to_str().unwrap(), "/run/notred.sock");
}
