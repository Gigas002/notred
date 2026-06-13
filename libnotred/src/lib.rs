//! notred platform library: IPC wire types, Unix-socket server, FDN host.

#[cfg(feature = "ipc")]
pub mod ipc;
#[cfg(feature = "ipc")]
pub mod wire;

#[cfg(feature = "server")]
pub mod dbus;
#[cfg(feature = "server")]
pub mod host;
#[cfg(feature = "server")]
pub mod model;
#[cfg(feature = "server")]
pub mod queue;

#[cfg(feature = "ipc")]
pub use ipc::IpcError;

/// Library version (matches workspace package version at release time).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
