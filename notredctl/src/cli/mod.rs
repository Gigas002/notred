use clap::{Parser, Subcommand};

use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "notredctl",
    version,
    about = "Control connector for notred (IPC client)"
)]
pub struct Cli {
    /// Unix socket path (default: `$XDG_RUNTIME_DIR/notred.sock`).
    #[arg(long, global = true)]
    pub socket: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Health check (`ping` → `pong` on the wire).
    Ping,
    /// Stream NDJSON event lines from the daemon until killed.
    Subscribe,
    /// Active notifications snapshot as JSON on stdout.
    List,
    /// Dismiss one active notification by id.
    Close {
        /// Notification id to dismiss.
        id: u32,
    },
    /// Dismiss all active notifications.
    #[command(name = "close-all")]
    CloseAll,
    /// Re-read `notred` config from disk.
    Reload,
    /// Hold new notifications until `unpause`.
    Pause,
    /// Resume surfacing held notifications.
    Unpause,
    /// Invoke a notification action (emits FDN `ActionInvoked`).
    Activate {
        /// Notification id.
        id: u32,
        /// Action key (defaults to `default` when omitted).
        key: Option<String>,
    },
    /// Report a pointer gesture (`button_left`, `button_middle`, `button_right`, `touch`).
    Input {
        /// Notification id.
        id: u32,
        /// Event kind (see `docs/IPC.md`).
        event_kind: String,
    },
    /// Session history rows as JSON on stdout (`history` feature).
    #[cfg(feature = "history")]
    #[command(name = "list-history")]
    ListHistory,
    /// Remove from history; dismiss on FDN if still active (`history` feature).
    #[cfg(feature = "history")]
    Remove {
        /// Notification id.
        id: u32,
    },
}

#[cfg(test)]
mod tests;
