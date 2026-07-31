use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("transport has not been started")]
    TransportNotStarted,
    #[error("SDK lifecycle lock is poisoned")]
    LifecyclePoisoned,
    #[error("request timed out: {0}")]
    RequestTimeout(String),
    #[error("RPC error {code}: {message}")]
    Rpc {
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
    #[error("Autohand authentication is required: {message}")]
    AuthenticationRequired {
        code: i64,
        message: String,
        retryable: bool,
        provider_id: Option<String>,
    },
    #[error("Autohand CLI initialization failed during {stage}: {message}")]
    InitializationFailed {
        code: i64,
        message: String,
        stage: String,
        retryable: bool,
    },
    #[error("structured output error: {0}")]
    StructuredOutput(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("unsupported answer contract version {observed}; expected {expected}")]
    UnsupportedContractVersion { expected: u16, observed: u16 },
    #[error("unsupported Autohand login contract version {observed}; expected {expected}")]
    UnsupportedLoginContractVersion { expected: u16, observed: u16 },
    #[error("classified answer inference destination {destination} is blocked")]
    InferenceDestinationBlocked { destination: &'static str },
    #[error("Autohand login setup failed: {problem}")]
    LoginFailed { problem: crate::LoginProblem },
    #[error("{stream} exceeded its {limit}-byte limit")]
    OutputLimitExceeded { stream: &'static str, limit: usize },
    #[error("captured event count exceeded its {limit}-event limit")]
    EventLimitExceeded { limit: usize },
    #[error("event consumer fell behind and lost {count} events")]
    EventStreamLagged { count: u64 },
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("{profile} profile does not allow RPC method {method}")]
    ProfileViolation {
        profile: &'static str,
        method: String,
    },
    #[error("child network policy unavailable: {0}")]
    NetworkPolicyUnavailable(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("channel closed")]
    ChannelClosed,
    #[error("agent stream ended without a terminal event")]
    MissingTerminalEvent,
    #[error("agent run ended with {status}")]
    RunTerminated { status: crate::RunStatus },
    #[error("task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, Error>;
