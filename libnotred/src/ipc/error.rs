use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unexpected IPC response: {0}")]
    UnexpectedResponse(&'static str),
}
