use libnotred::ipc::Server;

use crate::config::Config;
use crate::error::NotredBinError;

pub async fn run(config: &Config) -> Result<(), NotredBinError> {
    if crate::ping::ping(&config.socket_path).await.is_ok() {
        return Err(NotredBinError::AlreadyRunning);
    }

    let server = Server::new(&config.socket_path);
    server.run().await?;
    Ok(())
}
