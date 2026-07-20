use serde::{Deserialize, Serialize};

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
