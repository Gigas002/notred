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

fn config_with_missing_fragment() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notred.toml");
    std::fs::write(
        &path,
        "[paths]\noverrides = [\"apps/missing/config.toml\"]\n[events]\non_notify = [\"true\"]\n",
    )
    .unwrap();
    (dir, path)
}

#[test]
fn a_missing_fragment_is_a_startup_problem_but_base_hooks_stay() {
    let (_dir, path) = config_with_missing_fragment();
    let file = FileConfig::load(Some(&path)).unwrap();
    let cli = Cli::try_parse_from(["notred"]).unwrap();
    let settings = resolve(&cli, file);
    let problems = settings.problems.join("\n");
    assert!(problems.contains("apps/missing/config.toml"), "{problems}");
    assert!(
        problems.contains("no override fragment is used"),
        "{problems}"
    );
    assert!(settings.runtime.events.base.on_notify.is_some());
}

#[test]
fn a_missing_fragment_fails_a_reload() {
    let (_dir, path) = config_with_missing_fragment();
    let err = super::runtime_from_path(&path).unwrap_err().to_string();
    assert!(err.contains("apps/missing/config.toml"), "{err}");
}
