use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "notred-tui",
    version,
    about = "Terminal notification manager for notred (uses notredctl only)"
)]
pub struct Cli {
    /// Path to `notredctl` (default: search `$PATH`).
    #[arg(long, default_value = "notredctl")]
    pub ctl: PathBuf,

    /// Unix socket path passed through to `notredctl --socket`.
    #[arg(long)]
    pub socket: Option<PathBuf>,
}

#[cfg(test)]
mod tests;
