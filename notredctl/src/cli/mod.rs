use clap::{Parser, Subcommand};

use std::io::{self, Write};
use std::path::PathBuf;

use crate::error::CtlError;

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

impl Cli {
    pub async fn run(self) -> Result<(), CtlError> {
        let socket_path = self.socket.map_or_else(default_socket_path, Ok)?;
        let mut client = libnotred::ipc::Client::connect(&socket_path)
            .await
            .map_err(|_| CtlError::DaemonUnreachable(socket_path.display().to_string()))?;

        match self.command {
            Command::Ping => {
                client.ping().await?;
                println!("pong");
            }
            Command::List => {
                let items = client.list().await?;
                let json = serde_json::to_string_pretty(&items).map_err(CtlError::Json)?;
                println!("{json}");
            }
            Command::Subscribe => {
                client
                    .subscribe(|line| {
                        let mut out = io::stdout().lock();
                        writeln!(out, "{line}").map_err(libnotred::IpcError::from)?;
                        out.flush().map_err(libnotred::IpcError::from)?;
                        Ok(())
                    })
                    .await?;
            }
            Command::Close { id } => {
                client.dismiss(id).await.map_err(|e| match e {
                    libnotred::IpcError::ServerError(msg) => CtlError::Server(msg),
                    other => CtlError::Ipc(other),
                })?;
                println!("closed {id}");
            }
            Command::CloseAll => {
                client.close_all().await?;
                println!("closed all");
            }
            Command::Reload => {
                client.reload().await.map_err(map_server_err)?;
                println!("reloaded");
            }
            Command::Pause => {
                client.pause().await.map_err(map_server_err)?;
                println!("paused");
            }
            Command::Unpause => {
                client.unpause().await.map_err(map_server_err)?;
                println!("unpaused");
            }
            Command::Activate { id, key } => {
                client
                    .activate(id, key.as_deref())
                    .await
                    .map_err(map_server_err)?;
                println!("activated {id}");
            }
            #[cfg(feature = "history")]
            Command::ListHistory => {
                let rows = client.list_history(None, None, None).await.map_err(map_server_err)?;
                let json = serde_json::to_string_pretty(&rows).map_err(CtlError::Json)?;
                println!("{json}");
            }
            #[cfg(feature = "history")]
            Command::Remove { id } => {
                client.remove(id).await.map_err(map_server_err)?;
                println!("removed {id}");
            }
        }
        Ok(())
    }
}

fn map_server_err(e: libnotred::IpcError) -> CtlError {
    match e {
        libnotred::IpcError::ServerError(msg) => CtlError::Server(msg),
        other => CtlError::Ipc(other),
    }
}

fn default_socket_path() -> Result<PathBuf, CtlError> {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|_| CtlError::DaemonUnreachable("XDG_RUNTIME_DIR not set".to_owned()))?;
    Ok(PathBuf::from(dir).join("notred.sock"))
}

#[cfg(test)]
mod tests;
