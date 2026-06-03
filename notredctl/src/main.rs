mod cli;
mod error;
mod logger;

use std::process::ExitCode;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> ExitCode {
    logger::init();

    match Cli::parse().run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(%err, "notredctl failed");
            ExitCode::from(1)
        }
    }
}
