//! XDG `notred.toml` config loader (deserialization only).

use std::path::{Path, PathBuf};

#[cfg(feature = "history")]
use libnotred::HistorySettings;
use serde::Deserialize;
use serde::de::DeserializeOwned;

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
    pub notifications: NotificationsConfig,

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

    /// Keys in the file that no setting uses, as messages to log.
    #[serde(skip)]
    pub unknown_keys: Vec<String>,
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

#[derive(Debug, Deserialize)]
pub struct NotificationsConfig {
    /// Advertise the FDN `body-markup` capability and tag outgoing
    /// notifications so subscribers (e.g. poshanka) render `body` as Pango
    /// markup instead of escaped plain text.
    #[serde(default = "default_body_markup")]
    pub body_markup: bool,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            body_markup: default_body_markup(),
        }
    }
}

fn default_body_markup() -> bool {
    true
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
            notifications: NotificationsConfig::default(),
            events: EventsConfig::default(),
            #[cfg(feature = "history")]
            history: HistoryConfig::default(),
            #[cfg(feature = "history")]
            history_path: default_history_path(),
            source_path: None,
            unknown_keys: Vec::new(),
        }
    }
}

impl FileConfig {
    /// Load from an explicit path, or discover via XDG, or return defaults.
    pub fn load(explicit: Option<&Path>) -> Result<Self, NotredBinError> {
        let path = match explicit {
            Some(path) => path.to_path_buf(),
            None => {
                let xdg = Self::xdg_path();
                if !xdg.exists() {
                    return Ok(FileConfig::default());
                }
                xdg
            }
        };
        let mut unknown_keys = Vec::new();
        let mut cfg: FileConfig = read_toml(&path, &mut unknown_keys)?;
        cfg.source_path = Some(path);
        cfg.unknown_keys = unknown_keys;
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

/// Read and parse the TOML file at `path`, adding a message to `unknown`
/// for every key that no setting uses (a typo would otherwise be ignored).
pub(crate) fn read_toml<T: DeserializeOwned>(
    path: &Path,
    unknown: &mut Vec<String>,
) -> Result<T, NotredBinError> {
    let config_err = |e: &dyn std::fmt::Display| {
        NotredBinError::Config(format!("{}: {}", path.display(), e.to_string().trim_end()))
    };
    let text = std::fs::read_to_string(path).map_err(|e| config_err(&e))?;
    toml::Deserializer::parse(&text)
        .and_then(|de| {
            serde_ignored::deserialize(de, |key| {
                unknown.push(format!(
                    "{}: unknown key `{}` ignored",
                    path.display(),
                    key_path(&key)
                ));
            })
        })
        .map_err(|e| config_err(&e))
}

/// `a.b[0].c` for a serde_ignored path.
fn key_path(path: &serde_ignored::Path<'_>) -> String {
    use serde_ignored::Path;
    match path {
        Path::Root => String::new(),
        Path::Seq { parent, index } => format!("{}[{index}]", key_path(parent)),
        Path::Map { parent, key } => match key_path(parent) {
            p if p.is_empty() => key.clone(),
            p => format!("{p}.{key}"),
        },
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => key_path(parent),
    }
}

#[cfg(test)]
mod tests;
