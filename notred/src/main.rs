mod app;
mod cli;
mod commands;
mod config;
mod error;
mod logger;
mod settings;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::config::FileConfig;
use crate::settings::resolve;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let file = match FileConfig::load(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    let settings = resolve(&cli, file);
    logger::init(&settings.log_filter);

    match app::run(cli.command, &settings).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(%err, "notred failed");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod main_tests {
    use clap::Parser;

    use crate::cli::Cli;
    use crate::config::FileConfig;
    use crate::settings::resolve;

    #[test]
    fn resolve_from_defaults() {
        let cli = Cli::try_parse_from(["notred"]).unwrap();
        let settings = resolve(&cli, FileConfig::default());
        assert!(settings.socket_path.ends_with("notred.sock"));
    }
}
