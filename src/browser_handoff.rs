use serde::{Deserialize, Serialize};

/// Optional browser-extension routing used when creating a handoff.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserHandoffCreateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_url: Option<String>,
}

/// Browser handoff metadata returned by the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserHandoffCreateResult {
    pub token: String,
    pub session_id: String,
    pub workspace_root: String,
    pub created_at: String,
    pub expires_at: String,
    pub url: String,
}
