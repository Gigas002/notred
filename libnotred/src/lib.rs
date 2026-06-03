//! notred platform library: IPC wire types and Unix-socket server.

pub mod ipc;
pub mod wire;

pub use ipc::IpcError;

/// Library version (matches workspace package version at release time).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
