use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("transport has not been started")]
    TransportNotStarted,
    #[error("request timed out: {0}")]
    RequestTimeout(String),
    #[error("RPC error {code}: {message}")]
    Rpc {
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
    #[error("structured output error: {0}")]
    StructuredOutput(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("channel closed")]
    ChannelClosed,
    #[error("task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, Error>;
