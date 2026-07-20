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
