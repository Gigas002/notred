//! Merge CLI, config file, and defaults.

use std::path::PathBuf;

use crate::cli::Cli;
use crate::config::FileConfig;

#[derive(Debug, Clone)]
pub struct Settings {
    pub ctl: PathBuf,
    pub socket: Option<PathBuf>,
}

pub fn resolve(cli: &Cli, file: FileConfig) -> Settings {
    Settings {
        ctl: file.ctl.unwrap_or_else(|| cli.ctl.clone()),
        socket: cli.socket.clone().or(file.socket),
    }
}

#[cfg(test)]
mod tests;
