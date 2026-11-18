use clap::Parser;

use super::Cli;

#[test]
fn parses_defaults() {
    let cli = Cli::try_parse_from(["notred-tui"]).unwrap();
    assert!(cli.ctl.is_none());
    assert!(cli.socket.is_none());
}

#[test]
fn socket_override_parses() {
    let cli = Cli::try_parse_from(["notred-tui", "--socket", "/tmp/s.sock"]).unwrap();
    assert_eq!(
        cli.socket.as_deref(),
        Some(std::path::Path::new("/tmp/s.sock"))
    );
}
