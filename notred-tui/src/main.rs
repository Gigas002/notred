mod app;
mod cli;
mod config;
mod ctl;
mod logger;
mod model;
mod settings;
mod ui;

use std::io;
use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::config::FileConfig;
use crate::settings::resolve;

fn main() -> ExitCode {
    let cli = Cli::parse();

    let file = match FileConfig::load(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    let settings = resolve(&cli, file);
    logger::init();

    match app::run(&settings) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(%err, "notred-tui failed");
            if err.kind() == io::ErrorKind::Unsupported {
                ExitCode::from(2)
            } else {
                ExitCode::from(1)
            }
        }
    }
}
