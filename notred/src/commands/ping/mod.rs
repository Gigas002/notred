use std::path::Path;

use libnotred::ipc::Client;

use crate::error::NotredBinError;

pub async fn run(socket_path: &Path) -> Result<(), NotredBinError> {
    let mut client = Client::connect(socket_path)
        .await
        .map_err(|_| NotredBinError::DaemonUnreachable(socket_path.display().to_string()))?;
    client.ping().await?;
    println!("pong");
    Ok(())
}
