//! Application entry and command dispatch.

mod daemon;

use crate::cli::Command;
use crate::commands;
use crate::error::NotredBinError;
use crate::settings::Settings;

pub async fn run(command: Option<Command>, settings: &Settings) -> Result<(), NotredBinError> {
    match command.unwrap_or(Command::Run) {
        Command::Run => daemon::run(settings).await,
        Command::Ping => commands::ping::run(&settings.socket_path).await,
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;
    use crate::config::FileConfig;
    use crate::settings::resolve;

    #[test]
    fn settings_pipeline_smoke() {
        let cli = Cli::try_parse_from(["notred", "ping"]).unwrap();
        let settings = resolve(&cli, FileConfig::default());
        assert!(settings.socket_path.ends_with("notred.sock"));
    }
}
