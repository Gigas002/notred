mod app;
mod cli;
mod error;
mod logger;
mod settings;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::settings::resolve;

#[tokio::main]
async fn main() -> ExitCode {
    logger::init();

    let cli = Cli::parse();
    let settings = match resolve(cli.socket, cli.command) {
        Ok(s) => s,
        Err(err) => {
            tracing::error!(%err, "notredctl failed");
            return ExitCode::from(1);
        }
    };

    match app::run(settings).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(%err, "notredctl failed");
            ExitCode::from(1)
        }
    }
}
