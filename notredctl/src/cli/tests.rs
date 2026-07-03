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

#[test]
fn close_subcommand_parses() {
    let cli = Cli::try_parse_from(["notredctl", "close", "42"]).unwrap();
    assert!(matches!(cli.command, Command::Close { id: 42 }));
}

#[test]
fn close_all_subcommand_parses() {
    let cli = Cli::try_parse_from(["notredctl", "close-all"]).unwrap();
    assert!(matches!(cli.command, Command::CloseAll));
}

#[test]
fn reload_subcommand_parses() {
    let cli = Cli::try_parse_from(["notredctl", "reload"]).unwrap();
    assert!(matches!(cli.command, Command::Reload));
}

#[test]
fn pause_unpause_subcommands_parse() {
    let pause = Cli::try_parse_from(["notredctl", "pause"]).unwrap();
    assert!(matches!(pause.command, Command::Pause));
    let unpause = Cli::try_parse_from(["notredctl", "unpause"]).unwrap();
    assert!(matches!(unpause.command, Command::Unpause));
}

#[test]
fn activate_subcommand_parses() {
    let cli = Cli::try_parse_from(["notredctl", "activate", "7"]).unwrap();
    assert!(matches!(
        cli.command,
        Command::Activate { id: 7, key: None }
    ));
    let with_key = Cli::try_parse_from(["notredctl", "activate", "7", "open"]).unwrap();
    assert!(matches!(
        with_key.command,
        Command::Activate {
            id: 7,
            key: Some(k)
        } if k == "open"
    ));
}
