//! Resolved settings for `notredctl` (CLI > defaults).

use std::path::PathBuf;

use crate::cli::Command;
use crate::error::CtlError;

#[derive(Debug, Clone)]
pub struct Settings {
    pub socket_path: PathBuf,
    pub command: Command,
}

pub fn resolve(socket: Option<PathBuf>, command: Command) -> Result<Settings, CtlError> {
    let socket_path = socket.map_or_else(default_socket_path, Ok)?;
    Ok(Settings {
        socket_path,
        command,
    })
}

fn default_socket_path() -> Result<PathBuf, CtlError> {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|_| CtlError::DaemonUnreachable("XDG_RUNTIME_DIR not set".to_owned()))?;
    Ok(PathBuf::from(dir).join("notred.sock"))
}

#[cfg(test)]
mod tests;
