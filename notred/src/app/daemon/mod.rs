use libnotred::host::{HostConfig, NotredHost};

use crate::error::NotredBinError;
use crate::settings::{Settings, runtime_from_path};

pub async fn run(settings: &Settings) -> Result<(), NotredBinError> {
    if crate::commands::ping::run(&settings.socket_path)
        .await
        .is_ok()
    {
        return Err(NotredBinError::AlreadyRunning);
    }

    let reload = settings.config_path.clone().map(|path| {
        let path = path.to_path_buf();
        std::sync::Arc::new(move || runtime_from_path(&path).map_err(|e| e.to_string()))
            as std::sync::Arc<dyn Fn() -> Result<libnotred::RuntimeConfig, String> + Send + Sync>
    });

    let host_config = HostConfig {
        socket_path: settings.socket_path.clone(),
        runtime: settings.runtime.clone(),
        #[cfg(feature = "history")]
        history_path: settings.history_path.clone(),
        reload,
    };
    let host = NotredHost::new(host_config);
    host.run().await?;
    Ok(())
}
