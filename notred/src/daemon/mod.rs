use libnotred::host::{HostConfig, NotredHost};
use libnotred::RuntimeConfig;

use crate::config::Config;
use crate::error::NotredBinError;

pub async fn run(config: &Config) -> Result<(), NotredBinError> {
    if crate::ping::ping(&config.socket_path).await.is_ok() {
        return Err(NotredBinError::AlreadyRunning);
    }

    let config_path = Config::resolved_path(config.explicit_config.as_deref());
    let reload = config_path.map(|path| {
        let path = path.to_path_buf();
        std::sync::Arc::new(move || {
            Config::load(Some(&path))
                .map(|c| c.runtime())
                .map_err(|e| e.to_string())
        }) as std::sync::Arc<dyn Fn() -> Result<RuntimeConfig, String> + Send + Sync>
    });

    let host_config = HostConfig {
        socket_path: config.socket_path.clone(),
        runtime: config.runtime(),
        #[cfg(feature = "history")]
        history_path: config.history_path.clone(),
        reload,
    };
    let host = NotredHost::new(host_config);
    host.run().await?;
    Ok(())
}
