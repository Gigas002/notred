//! Unix-socket NDJSON IPC (`docs/IPC.md`).

pub mod client;
pub mod codec;
pub mod error;
#[cfg(feature = "server")]
pub mod server;

pub use client::Client;
pub use error::IpcError;
#[cfg(feature = "server")]
pub use server::Server;

/// Default socket path: `$XDG_RUNTIME_DIR/notred.sock`.
pub fn default_socket_path() -> std::path::PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(dir).join("notred.sock")
}

#[cfg(test)]
mod tests;
