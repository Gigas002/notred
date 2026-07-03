//! XDG `notred.toml` config loader.

use std::path::{Path, PathBuf};

use libnotred::RuntimeConfig;
#[cfg(feature = "history")]
use libnotred::HistorySettings;
use serde::Deserialize;

use crate::error::NotredBinError;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Unix socket path for the IPC server.
    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,

    /// Tracing filter string (overridden by `RUST_LOG`).
    #[serde(default = "default_log_filter")]
    pub log_filter: String,

    #[serde(default)]
    pub events: EventsConfig,

    #[cfg(feature = "history")]
    #[serde(default)]
    pub history: HistoryConfig,

    /// History DB path (`history` feature).
    #[cfg(feature = "history")]
    #[serde(default = "default_history_path")]
    pub history_path: PathBuf,

    /// Set when loading from an explicit `--config` path (used for reload).
    #[serde(skip)]
    pub explicit_config: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
pub struct EventsConfig {
    /// Optional argv invoked on action activation (`NOTRED_*` env vars set).
    #[serde(default)]
    pub on_action: Option<Vec<String>>,
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

#[cfg(feature = "history")]
fn default_history_path() -> PathBuf {
    libnotred::history::default_history_path()
}

#[cfg(feature = "history")]
fn default_history_enabled() -> bool {
    true
}

#[cfg(feature = "history")]
fn default_history_flush() -> bool {
    true
}

#[cfg(feature = "history")]
fn default_max_entries() -> u32 {
    5
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            log_filter: default_log_filter(),
            events: EventsConfig::default(),
            #[cfg(feature = "history")]
            history: HistoryConfig::default(),
            #[cfg(feature = "history")]
            history_path: default_history_path(),
            explicit_config: None,
        }
    }
}

impl Config {
    /// Load from an explicit path, or discover via XDG, or return defaults.
    pub fn load(explicit: Option<&Path>) -> Result<Self, NotredBinError> {
        let cfg = if let Some(path) = explicit {
            let text = std::fs::read_to_string(path)
                .map_err(|e| NotredBinError::Config(format!("{}: {e}", path.display())))?;
            let mut cfg: Config = toml::from_str(&text)
                .map_err(|e| NotredBinError::Config(format!("{}: {e}", path.display())))?;
            cfg.explicit_config = Some(path.to_path_buf());
            cfg
        } else {
            let xdg = Self::xdg_path();
            if xdg.exists() {
                let text = std::fs::read_to_string(&xdg)
                    .map_err(|e| NotredBinError::Config(format!("{}: {e}", xdg.display())))?;
                let mut cfg: Config = toml::from_str(&text)
                    .map_err(|e| NotredBinError::Config(format!("{}: {e}", xdg.display())))?;
                cfg.explicit_config = Some(xdg);
                cfg
            } else {
                Config::default()
            }
        };
        Ok(cfg)
    }

    pub fn runtime(&self) -> RuntimeConfig {
        RuntimeConfig {
            on_action: self.events.on_action.clone(),
            #[cfg(feature = "history")]
            history: HistorySettings {
                enabled: self.history.enabled,
                flush: self.history.flush,
                max_entries: self.history.max_entries,
            },
        }
    }

    pub fn resolved_path(explicit: Option<&Path>) -> Option<PathBuf> {
        explicit.map(Path::to_path_buf).or_else(|| {
            let xdg = Self::xdg_path();
            xdg.exists().then_some(xdg)
        })
    }

    pub fn xdg_path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
            format!("{home}/.config")
        });
        PathBuf::from(base).join("notred").join("notred.toml")
    }
}

#[cfg(test)]
mod tests;
