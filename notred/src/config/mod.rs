//! XDG `notred.toml` config loader (deserialization only).

use std::path::{Path, PathBuf};

#[cfg(feature = "history")]
use libnotred::HistorySettings;
use serde::Deserialize;

use crate::error::NotredBinError;

pub mod events;
pub mod overrides;

pub use events::EventsConfig;
pub use overrides::load_events_policy;

/// On-disk config shape (`notred.toml`).
#[derive(Debug, Deserialize)]
pub struct FileConfig {
    /// Unix socket path for the IPC server.
    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,

    /// Tracing filter string (overridden by `RUST_LOG`).
    #[serde(default = "default_log_filter")]
    pub log_filter: String,

    #[serde(default)]
    pub paths: PathsConfig,

    #[serde(default)]
    pub queue: QueueConfig,

    #[serde(default)]
    pub events: EventsConfig,

    #[cfg(feature = "history")]
    #[serde(default)]
    pub history: HistoryConfig,

    /// History DB path (`history` feature).
    #[cfg(feature = "history")]
    #[serde(default = "default_history_path")]
    pub history_path: PathBuf,

    /// Set when loading from an explicit path (not serialized).
    #[serde(skip)]
    pub source_path: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PathsConfig {
    #[serde(default)]
    pub overrides: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct QueueConfig {
    /// Maximum active notifications (`0` = unlimited).
    #[serde(default)]
    pub max_visible: u32,
    /// Default auto-dismiss when FDN sends `expire_timeout = -1` (`0` = no timer).
    #[serde(default = "default_default_timeout_ms")]
    pub default_timeout_ms: u32,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_visible: 0,
            default_timeout_ms: default_default_timeout_ms(),
        }
    }
}

#[cfg(feature = "history")]
#[derive(Debug, Deserialize)]
pub struct HistoryConfig {
    #[serde(default = "default_history_enabled")]
    pub enabled: bool,
    #[serde(default = "default_history_flush")]
    pub flush: bool,
    #[serde(default = "default_max_entries")]
    pub max_entries: u32,
}

#[cfg(feature = "history")]
impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: default_history_enabled(),
            flush: default_history_flush(),
            max_entries: default_max_entries(),
        }
    }
}

fn default_socket_path() -> PathBuf {
    libnotred::ipc::default_socket_path()
}

fn default_log_filter() -> String {
    "warn".into()
}

fn default_default_timeout_ms() -> u32 {
    0
}

#[cfg(feature = "history")]
fn default_history_path() -> PathBuf {
    libnotred::history::default_history_path()
}

#[cfg(feature = "history")]
fn default_history_enabled() -> bool {
    false
}

#[cfg(feature = "history")]
fn default_history_flush() -> bool {
    true
}

#[cfg(feature = "history")]
fn default_max_entries() -> u32 {
    5
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            log_filter: default_log_filter(),
            paths: PathsConfig::default(),
            queue: QueueConfig::default(),
            events: EventsConfig::default(),
            #[cfg(feature = "history")]
            history: HistoryConfig::default(),
            #[cfg(feature = "history")]
            history_path: default_history_path(),
            source_path: None,
        }
    }
}

impl FileConfig {
    /// Load from an explicit path, or discover via XDG, or return defaults.
    pub fn load(explicit: Option<&Path>) -> Result<Self, NotredBinError> {
        let cfg = if let Some(path) = explicit {
            let text = std::fs::read_to_string(path)
                .map_err(|e| NotredBinError::Config(format!("{}: {e}", path.display())))?;
            let mut cfg: FileConfig = toml::from_str(&text)
                .map_err(|e| NotredBinError::Config(format!("{}: {e}", path.display())))?;
            cfg.source_path = Some(path.to_path_buf());
            cfg
        } else {
            let xdg = Self::xdg_path();
            if xdg.exists() {
                let text = std::fs::read_to_string(&xdg)
                    .map_err(|e| NotredBinError::Config(format!("{}: {e}", xdg.display())))?;
                let mut cfg: FileConfig = toml::from_str(&text)
                    .map_err(|e| NotredBinError::Config(format!("{}: {e}", xdg.display())))?;
                cfg.source_path = Some(xdg);
                cfg
            } else {
                FileConfig::default()
            }
        };
        Ok(cfg)
    }

    pub fn xdg_path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
            format!("{home}/.config")
        });
        PathBuf::from(base).join("notred").join("notred.toml")
    }

    pub fn config_dir(&self) -> Option<PathBuf> {
        self.source_path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
    }

    #[cfg(feature = "history")]
    pub(crate) fn history_settings(&self) -> HistorySettings {
        HistorySettings {
            enabled: self.history.enabled,
            flush: self.history.flush,
            max_entries: self.history.max_entries,
        }
    }
}

#[cfg(test)]
mod tests;
