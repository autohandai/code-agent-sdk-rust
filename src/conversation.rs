use serde::{Deserialize, Serialize};

/// Result returned after replacing the active conversation with a new session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetResult {
    pub session_id: String,
}
