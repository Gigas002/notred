//! Behavior override fragments (`paths.overrides` — poshanka-parity layout).

use std::path::Path;

use libnotred::wire::Urgency;
use libnotred::{EventsPolicy, LoadedEventOverride, OverrideKind};
use serde::Deserialize;

use super::events::EventsConfig;
use crate::error::NotredBinError;

#[derive(Debug, Clone, Deserialize)]
pub struct FragmentConfig {
    #[serde(rename = "override")]
    pub override_meta: OverrideMeta,
    #[serde(default)]
    pub paths: Option<FragmentPaths>,
    #[serde(default)]
    pub events: EventsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OverrideMeta {
    #[serde(rename = "type")]
    pub kind: OverrideType,
    pub name: Option<String>,
    pub level: Option<UrgencyLevel>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OverrideType {
    App,
    Urgency,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UrgencyLevel {
    Low,
    Normal,
    Critical,
}

impl From<UrgencyLevel> for Urgency {
    fn from(level: UrgencyLevel) -> Self {
        match level {
            UrgencyLevel::Low => Urgency::Low,
            UrgencyLevel::Normal => Urgency::Normal,
            UrgencyLevel::Critical => Urgency::Critical,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FragmentPaths {
    #[serde(default)]
    pub overrides: Vec<String>,
}

impl FragmentConfig {
    pub fn load(path: &Path) -> Result<Self, NotredBinError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| NotredBinError::Config(format!("{}: {e}", path.display())))?;
        toml::from_str(&text)
            .map_err(|e| NotredBinError::Config(format!("{}: {e}", path.display())))
    }
}

/// Build runtime [`EventsPolicy`] from root config + override fragments on disk.
pub fn load_events_policy(
    base_events: &EventsConfig,
    override_paths: &[String],
    config_dir: &Path,
) -> Result<EventsPolicy, NotredBinError> {
    let overrides = override_paths
        .iter()
        .map(|rel| load_single_override(&config_dir.join(rel)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EventsPolicy {
        base: base_events.to_hooks(),
        overrides,
    })
}

fn load_single_override(fragment_path: &Path) -> Result<LoadedEventOverride, NotredBinError> {
    let config = FragmentConfig::load(fragment_path)?;
    let dir = fragment_path.parent().unwrap_or(Path::new(""));

    let kind = match config.override_meta.kind {
        OverrideType::App => {
            let name = config.override_meta.name.ok_or_else(|| {
                NotredBinError::Config(format!(
                    "{}: app override requires `name`",
                    fragment_path.display()
                ))
            })?;
            OverrideKind::App { name }
        }
        OverrideType::Urgency => {
            let level = config.override_meta.level.ok_or_else(|| {
                NotredBinError::Config(format!(
                    "{}: urgency override requires `level`",
                    fragment_path.display()
                ))
            })?;
            OverrideKind::Urgency {
                level: level.into(),
            }
        }
    };

    let nested = config
        .paths
        .as_ref()
        .map(|p| p.overrides.as_slice())
        .unwrap_or(&[])
        .iter()
        .map(|rel| load_single_override(&dir.join(rel)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LoadedEventOverride {
        kind,
        hooks: config.events.to_hooks(),
        nested,
    })
}

#[cfg(test)]
mod tests;
