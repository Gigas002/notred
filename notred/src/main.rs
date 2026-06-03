mod cli;
mod config;
mod daemon;
mod error;
mod logger;
mod ping;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::config::Config;
use crate::error::NotredBinError;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let config = match Config::load(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    logger::init(&config.log_filter);

    match run(cli.command, config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(%err, "notred failed");
            ExitCode::from(1)
        }
    }
}

async fn run(command: Option<Command>, config: Config) -> Result<(), NotredBinError> {
    match command.unwrap_or(Command::Run) {
        Command::Run => daemon::run(&config).await,
        Command::Ping => ping::ping(&config.socket_path).await,
    }
}
