use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "notred-tui",
    version,
    about = "Terminal notification manager for notred (uses notredctl only)"
)]
pub struct Cli {
    /// Path to `notredctl` (default: `ctl` in tui.toml, else search `$PATH`).
    #[arg(long)]
    pub ctl: Option<PathBuf>,

    /// Unix socket path passed through to `notredctl --socket`.
    #[arg(long)]
    pub socket: Option<PathBuf>,

    /// Optional config file (default: `$XDG_CONFIG_HOME/notred/tui.toml`).
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[cfg(test)]
mod tests;
