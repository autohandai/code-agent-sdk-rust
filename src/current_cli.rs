use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

/// Result returned after acknowledging receipt of a permission request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionAcknowledgedResult {
    pub success: bool,
}

/// Result returned after allowing or denying a directory access request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryAccessResponseResult {
    pub success: bool,
}

/// Result returned after acknowledging a directory access request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryAccessAcknowledgedResult {
    pub success: bool,
}

/// Disposition of a pending multi-file preview batch. The tagged enum keeps
/// selected change IDs exclusive to the `accept_selected` action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ChangesDecisionParams {
    AcceptAll {
        #[serde(rename = "batchId")]
        batch_id: String,
    },
    RejectAll {
        #[serde(rename = "batchId")]
        batch_id: String,
    },
    AcceptSelected {
        #[serde(rename = "batchId")]
        batch_id: String,
        #[serde(rename = "selectedChangeIds")]
        selected_change_ids: Vec<String>,
    },
}

impl ChangesDecisionParams {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        let (batch_id, selected) = match self {
            Self::AcceptAll { batch_id } | Self::RejectAll { batch_id } => (batch_id, None),
            Self::AcceptSelected {
                batch_id,
                selected_change_ids,
            } => (batch_id, Some(selected_change_ids)),
        };
        if batch_id.trim().is_empty() {
            return Err("changes batch_id is required");
        }
        if let Some(selected) = selected {
            if selected.is_empty() {
                return Err("accept_selected requires at least one change ID");
            }
            if selected.iter().any(|id| id.trim().is_empty()) {
                return Err("selected change IDs cannot be blank");
            }
        }
        Ok(())
    }
}

/// Failure to apply one proposed change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChangesDecisionError {
    pub change_id: String,
    pub error: String,
}

/// Summary returned after applying or rejecting a preview batch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChangesDecisionResult {
    pub success: bool,
    pub applied_count: u64,
    pub skipped_count: u64,
    #[serde(default)]
    pub errors: Vec<ChangesDecisionError>,
}

/// Pagination controls for stored session history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetHistoryParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<NonZeroU64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<NonZeroU64>,
}

/// Stored session state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionHistoryStatus {
    Active,
    Completed,
    Crashed,
}

/// Summary of one stored CLI session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistoryEntry {
    pub session_id: String,
    pub created_at: String,
    pub last_active_at: String,
    pub project_name: String,
    pub model: String,
    pub message_count: u64,
    pub status: SessionHistoryStatus,
}

/// One page of stored CLI sessions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetHistoryResult {
    pub sessions: Vec<SessionHistoryEntry>,
    pub current_page: u64,
    pub total_pages: u64,
    pub total_items: u64,
}
