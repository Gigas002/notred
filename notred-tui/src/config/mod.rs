//! Optional `tui.toml` loader (`$XDG_CONFIG_HOME/notred/tui.toml`).

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct FileConfig {
    /// Path to `notredctl` (default: search `$PATH` for `notredctl`).
    pub ctl: Option<PathBuf>,
    /// Default `--socket` passed to every `notredctl` invocation.
    pub socket: Option<PathBuf>,
}

impl FileConfig {
    pub fn load(explicit: Option<&Path>) -> Result<Self, std::io::Error> {
        let path = explicit
            .map(Path::to_path_buf)
            .unwrap_or_else(Self::xdg_path);
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)?;
        toml::from_str(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn xdg_path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
            format!("{home}/.config")
        });
        PathBuf::from(base).join("notred").join("tui.toml")
    }
}

#[cfg(test)]
mod tests;
