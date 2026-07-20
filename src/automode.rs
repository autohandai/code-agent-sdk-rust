use serde::{Deserialize, Serialize};

/// Configuration accepted by `autohand.automode.start`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomodeStartParams {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_promise: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_worktree: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_interval: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_runtime: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cost: Option<f64>,
}

/// Acceptance result for an auto-mode session start request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomodeStartResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Persisted auto-mode lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutomodeStatus {
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomodeCheckpoint {
    pub commit: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomodeState {
    pub session_id: String,
    pub status: AutomodeStatus,
    pub current_iteration: u32,
    pub max_iterations: u32,
    pub files_created: u32,
    pub files_modified: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checkpoint: Option<AutomodeCheckpoint>,
}

/// Runtime flags and optional persisted state for auto-mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomodeStatusResult {
    pub active: bool,
    pub paused: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<AutomodeState>,
}

/// Result of pausing an active auto-mode session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomodePauseResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of resuming a paused auto-mode session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomodeResumeResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Optional caller context for cancelling auto-mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomodeCancelParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Result of cancelling an active auto-mode session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomodeCancelResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
