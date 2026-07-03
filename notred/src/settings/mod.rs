//! Merge CLI, config file, and defaults into a single `Settings` object.

use std::path::{Path, PathBuf};

use libnotred::{EventsPolicy, RuntimeConfig};

use crate::cli::Cli;
use crate::config::{FileConfig, load_events_policy};
use crate::error::NotredBinError;

/// Fully resolved runtime settings (only type passed below this boundary).
#[derive(Debug, Clone)]
pub struct Settings {
    pub socket_path: PathBuf,
    pub log_filter: String,
    pub runtime: RuntimeConfig,
    /// Path used for `reload` when the daemon was started with a config file.
    pub config_path: Option<PathBuf>,
    #[cfg(feature = "history")]
    pub history_path: PathBuf,
}

pub fn resolve(_cli: &Cli, file: FileConfig) -> Settings {
    let runtime = runtime_from_file(&file);
    Settings {
        socket_path: file.socket_path,
        log_filter: file.log_filter,
        runtime,
        config_path: file.source_path,
        #[cfg(feature = "history")]
        history_path: file.history_path,
    }
}

fn runtime_from_file(file: &FileConfig) -> RuntimeConfig {
    let events = events_policy_from_file(file);
    RuntimeConfig {
        max_visible: file.queue.max_visible,
        default_timeout_ms: file.queue.default_timeout_ms,
        events,
        #[cfg(feature = "history")]
        history: file.history_settings(),
    }
}

fn events_policy_from_file(file: &FileConfig) -> EventsPolicy {
    if let Some(dir) = file.config_dir() {
        load_events_policy(&file.events, &file.paths.overrides, &dir).unwrap_or_else(|e| {
            tracing::warn!(%e, "failed to load event override fragments");
            EventsPolicy {
                base: file.events.to_hooks(),
                overrides: vec![],
            }
        })
    } else {
        EventsPolicy {
            base: file.events.to_hooks(),
            overrides: vec![],
        }
    }
}

/// Reload runtime policy from a config file path (used by the daemon `reload` RPC).
pub fn runtime_from_path(path: &Path) -> Result<RuntimeConfig, NotredBinError> {
    let file = FileConfig::load(Some(path))?;
    Ok(runtime_from_file(&file))
}

#[cfg(test)]
mod tests;
