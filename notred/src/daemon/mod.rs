use libnotred::host::{HostConfig, NotredHost};

use crate::config::Config;
use crate::error::NotredBinError;

pub async fn run(config: &Config) -> Result<(), NotredBinError> {
    if crate::ping::ping(&config.socket_path).await.is_ok() {
        return Err(NotredBinError::AlreadyRunning);
    }

    let host_config = HostConfig {
        socket_path: config.socket_path.clone(),
    };
    let host = NotredHost::new(host_config);
    host.run().await?;
    Ok(())
}
