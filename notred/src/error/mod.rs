use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotredBinError {
    #[error(transparent)]
    Ipc(#[from] libnotred::IpcError),

    #[error(transparent)]
    Host(#[from] libnotred::host::HostError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("config error: {0}")]
    Config(String),

    #[error("daemon already running")]
    AlreadyRunning,

    #[error("daemon not reachable at {0}")]
    DaemonUnreachable(String),
}

#[cfg(test)]
mod tests;
