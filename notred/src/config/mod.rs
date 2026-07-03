//! XDG `notred.toml` config loader.

use std::path::{Path, PathBuf};

use libnotred::RuntimeConfig;
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

fn default_socket_path() -> PathBuf {
    libnotred::ipc::default_socket_path()
}

fn default_log_filter() -> String {
    "warn".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            log_filter: default_log_filter(),
            events: EventsConfig::default(),
            explicit_config: None,
        }
    }
}

impl Config {
    /// Load from an explicit path, or discover via XDG, or return defaults.
    ///
    /// - Explicit path given → error if missing or unparsable.
    /// - XDG path exists → parse it; error if unparsable.
    /// - Nothing found → silent defaults.
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
        }
    }

    /// Config file path used for `reload`, if any.
    pub fn resolved_path(explicit: Option<&Path>) -> Option<PathBuf> {
        explicit.map(Path::to_path_buf).or_else(|| {
            let xdg = Self::xdg_path();
            xdg.exists().then_some(xdg)
        })
    }

    /// `$XDG_CONFIG_HOME/notred/notred.toml` (falls back to `~/.config/…`).
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
