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
    /// Config mistakes found while resolving, to log once logging is set up.
    pub problems: Vec<String>,
}

pub fn resolve(_cli: &Cli, file: FileConfig) -> Settings {
    let mut problems = file.unknown_keys.clone();
    let events = events_policy(&file, &mut problems).unwrap_or_else(|e| {
        problems.push(format!("{e}; no override fragment is used"));
        base_policy(&file)
    });
    Settings {
        runtime: runtime(&file, events),
        socket_path: file.socket_path,
        log_filter: file.log_filter,
        config_path: file.source_path,
        #[cfg(feature = "history")]
        history_path: file.history_path,
        problems,
    }
}

fn runtime(file: &FileConfig, events: EventsPolicy) -> RuntimeConfig {
    RuntimeConfig {
        max_visible: file.queue.max_visible,
        default_timeout_ms: file.queue.default_timeout_ms,
        events,
        body_markup: file.notifications.body_markup,
        #[cfg(feature = "history")]
        history: file.history_settings(),
    }
}

fn base_policy(file: &FileConfig) -> EventsPolicy {
    EventsPolicy {
        base: file.events.to_hooks(),
        overrides: vec![],
    }
}

/// The `[events]` hooks plus the override fragments, which are found
/// relative to the config file (none without one).
fn events_policy(
    file: &FileConfig,
    problems: &mut Vec<String>,
) -> Result<EventsPolicy, NotredBinError> {
    match file.config_dir() {
        Some(dir) => load_events_policy(&file.events, &file.paths.overrides, &dir, problems),
        None => Ok(base_policy(file)),
    }
}

/// Reload runtime policy from a config file path (used by the daemon `reload` RPC).
///
/// Stricter than start-up: a fragment that can't be loaded fails the reload,
/// so the running policy stays, instead of silently dropping every fragment.
/// Unknown keys are logged.
pub fn runtime_from_path(path: &Path) -> Result<RuntimeConfig, NotredBinError> {
    let file = FileConfig::load(Some(path))?;
    let mut problems = file.unknown_keys.clone();
    let events = events_policy(&file, &mut problems)?;
    for problem in &problems {
        tracing::warn!("{problem}");
    }
    Ok(runtime(&file, events))
}

#[cfg(test)]
mod tests;
