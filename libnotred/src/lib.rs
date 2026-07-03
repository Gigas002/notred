//! notred platform library: IPC wire types, Unix-socket server, FDN host.

#[cfg(feature = "ipc")]
pub mod ipc;
#[cfg(feature = "ipc")]
pub mod wire;

#[cfg(feature = "server")]
pub mod dbus;
#[cfg(feature = "server")]
pub mod events;
#[cfg(feature = "history")]
pub mod history;
#[cfg(feature = "server")]
pub mod host;
#[cfg(feature = "server")]
pub mod model;
#[cfg(feature = "server")]
pub mod queue;
#[cfg(feature = "server")]
pub mod spawn;
#[cfg(feature = "server")]
pub mod timeouts;

#[cfg(feature = "ipc")]
pub use ipc::IpcError;

#[cfg(feature = "server")]
pub use events::{EventsHooks, EventsPolicy, LoadedEventOverride, OverrideKind};
#[cfg(feature = "history")]
pub use host::state::HistorySettings;
#[cfg(feature = "server")]
pub use host::state::RuntimeConfig;

/// Library version (matches workspace package version at release time).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
