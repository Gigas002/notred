use clap::Parser;

use super::{Cli, Command};

#[test]
fn ping_subcommand_parses() {
    let cli = Cli::try_parse_from(["notredctl", "ping"]).unwrap();
    assert!(matches!(cli.command, Command::Ping));
}

#[test]
fn subscribe_subcommand_parses() {
    let cli = Cli::try_parse_from(["notredctl", "subscribe"]).unwrap();
    assert!(matches!(cli.command, Command::Subscribe));
}

#[test]
fn list_subcommand_parses() {
    let cli = Cli::try_parse_from(["notredctl", "list"]).unwrap();
    assert!(matches!(cli.command, Command::List));
}

#[test]
fn global_socket_override_parses() {
    let cli = Cli::try_parse_from(["notredctl", "--socket", "/run/notred.sock", "ping"]).unwrap();
    assert_eq!(
        cli.socket.as_deref(),
        Some(std::path::Path::new("/run/notred.sock"))
    );
}
