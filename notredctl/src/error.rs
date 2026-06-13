use thiserror::Error;

#[derive(Debug, Error)]
pub enum CtlError {
    #[error(transparent)]
    Ipc(#[from] libnotred::IpcError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("cannot reach notred daemon at {0}")]
    DaemonUnreachable(String),

    #[error("server error: {0}")]
    Server(String),
}
