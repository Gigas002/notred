use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
        }
        Ok(())
    }
}

fn default_socket_path() -> Result<PathBuf, CtlError> {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|_| CtlError::DaemonUnreachable("XDG_RUNTIME_DIR not set".to_owned()))?;
    Ok(PathBuf::from(dir).join("notred.sock"))
}

#[cfg(test)]
mod tests;
