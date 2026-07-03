use clap::Parser;

use super::resolve;

use crate::cli::Cli;
use crate::config::FileConfig;

#[test]
fn resolve_uses_file_fields() {
    let file = FileConfig {
        socket_path: "/tmp/custom.sock".into(),
        log_filter: "debug".into(),
        ..Default::default()
    };
    let cli = Cli::try_parse_from(["notred"]).unwrap();
    let settings = resolve(&cli, file);
    assert_eq!(settings.socket_path.to_str().unwrap(), "/tmp/custom.sock");
    assert_eq!(settings.log_filter, "debug");
    assert!(settings.config_path.is_none());
}

#[test]
fn resolve_preserves_config_path() {
    let file = FileConfig {
        source_path: Some("/etc/notred.toml".into()),
        ..Default::default()
    };
    let cli = Cli::try_parse_from(["notred"]).unwrap();
    let settings = resolve(&cli, file);
    assert_eq!(
        settings.config_path.as_deref(),
        Some(std::path::Path::new("/etc/notred.toml"))
    );
}

#[cfg(feature = "history")]
#[test]
fn resolve_history_disabled_by_default() {
    let file = FileConfig::default();
    let cli = Cli::try_parse_from(["notred"]).unwrap();
    let settings = resolve(&cli, file);
    assert!(!settings.runtime.history.enabled);
}
