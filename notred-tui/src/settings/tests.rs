use clap::Parser;

use super::resolve;

use crate::cli::Cli;
use crate::config::FileConfig;

#[test]
fn cli_socket_overrides_file() {
    let cli = Cli::try_parse_from([
        "notred-tui",
        "--socket",
        "/cli.sock",
        "--config",
        "/dev/null",
    ])
    .unwrap();
    let file = FileConfig {
        socket: Some("/file.sock".into()),
        ..Default::default()
    };
    let settings = resolve(&cli, file);
    assert_eq!(
        settings.socket.as_deref().unwrap().to_str().unwrap(),
        "/cli.sock"
    );
}

#[test]
fn file_ctl_used_when_present() {
    let cli = Cli::try_parse_from(["notred-tui"]).unwrap();
    let file = FileConfig {
        ctl: Some("/opt/notredctl".into()),
        ..Default::default()
    };
    let settings = resolve(&cli, file);
    assert_eq!(settings.ctl.to_str().unwrap(), "/opt/notredctl");
}
