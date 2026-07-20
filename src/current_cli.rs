use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
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

/// Role stored for a session message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RpcMessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Tool call stored inside an assistant message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RpcToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Map<String, Value>,
}

/// Message stored in a persisted session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RpcMessage {
    pub id: String,
    pub role: RpcMessageRole,
    pub content: String,
    pub timestamp: String,
    #[serde(default)]
    pub tool_calls: Vec<RpcToolCall>,
}

/// Complete payload for a successfully loaded session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetails {
    pub session_id: String,
    pub project_name: String,
    pub model: String,
    pub message_count: u64,
    pub status: String,
    pub created_at: String,
    pub last_active_at: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub messages: Vec<RpcMessage>,
    pub workspace_root: String,
}

/// A stored-session lookup is either complete details or an explicit failure.
/// Custom deserialization rejects partial success payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionLookupResult {
    Success(SessionDetails),
    Failure { error: Option<String> },
}

impl<'de> Deserialize<'de> for SessionLookupResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let success = value
            .get("success")
            .and_then(Value::as_bool)
            .ok_or_else(|| D::Error::custom("missing boolean success discriminator"))?;
        if success {
            let details = serde_json::from_value::<SessionDetails>(value)
                .map_err(|error| D::Error::custom(format!("invalid session details: {error}")))?;
            if details.session_id.trim().is_empty() || details.workspace_root.trim().is_empty() {
                return Err(D::Error::custom(
                    "session details require sessionId and workspaceRoot",
                ));
            }
            return Ok(Self::Success(details));
        }
        let error = value
            .get("error")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        Ok(Self::Failure { error })
    }
}

/// Result returned after restoring a stored session into the active process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionAttachResult {
    pub success: bool,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub message_count: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Timed unrestricted-mode settings. The current CLI treats any non-empty
/// pattern as enabled and an empty pattern as disabled.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct YoloSetParams {
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<NonZeroU64>,
}

/// Effective timed unrestricted-mode state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct YoloSetResult {
    pub success: bool,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

/// JSON Schema root type accepted by the CLI for VS Code MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpInputSchemaType {
    Object,
}

/// Object-shaped argument schema for a VS Code MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpInputSchema {
    #[serde(rename = "type")]
    pub schema_type: McpInputSchemaType,
    pub properties: serde_json::Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
}

/// Tool descriptor supplied by a VS Code extension.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpVsCodeTool {
    pub name: String,
    pub description: String,
    pub server_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<McpInputSchema>,
}

/// Replacement set of extension-provided MCP tools. An empty vector clears
/// the current set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpSetVsCodeToolsParams {
    pub tools: Vec<McpVsCodeTool>,
}

impl McpSetVsCodeToolsParams {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.tools.iter().any(|tool| {
            tool.name.trim().is_empty()
                || tool.description.trim().is_empty()
                || tool.server_name.trim().is_empty()
        }) {
            return Err("MCP tools require name, description, and server_name");
        }
        Ok(())
    }
}

/// Result returned after replacing VS Code MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpSetVsCodeToolsResult {
    pub success: bool,
}

/// Completion of an MCP invocation requested by the CLI. Separate variants
/// prevent ambiguous result-plus-error payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpInvocationResponseParams {
    Success {
        request_id: String,
        result: Option<String>,
    },
    Failure {
        request_id: String,
        error: String,
    },
}

impl McpInvocationResponseParams {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Success { request_id, .. } if request_id.trim().is_empty() => {
                Err("MCP invocation request_id is required")
            }
            Self::Failure { request_id, .. } if request_id.trim().is_empty() => {
                Err("MCP invocation request_id is required")
            }
            Self::Failure { error, .. } if error.trim().is_empty() => {
                Err("failed MCP invocation responses require an error")
            }
            _ => Ok(()),
        }
    }
}

impl Serialize for McpInvocationResponseParams {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Success { request_id, result } => {
                let mut payload = serde_json::Map::from_iter([
                    ("requestId".into(), Value::String(request_id.clone())),
                    ("success".into(), Value::Bool(true)),
                ]);
                if let Some(result) = result {
                    payload.insert("result".into(), Value::String(result.clone()));
                }
                Value::Object(payload).serialize(serializer)
            }
            Self::Failure { request_id, error } => serde_json::json!({
                "requestId": request_id,
                "success": false,
                "error": error,
            })
            .serialize(serializer),
        }
    }
}

/// Result returned after completing a pending MCP invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpInvocationResponseResult {
    pub success: bool,
}

/// Controls whether project learning recommendation performs a deeper scan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LearnRecommendParams {
    #[serde(default)]
    pub deep: bool,
}

/// Existing-skill concern found during learning analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LearningAuditStatus {
    Redundant,
    Outdated,
    Conflicting,
}

/// One existing-skill concern.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LearningAuditEntry {
    pub skill: String,
    pub status: LearningAuditStatus,
    pub reason: String,
}

/// Scored registry recommendation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LearningRecommendation {
    pub slug: String,
    pub score: f64,
    pub reason: String,
}

/// Project learning analysis and registry recommendations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LearnRecommendResult {
    pub success: bool,
    pub project_summary: String,
    pub audit: Vec<LearningAuditEntry>,
    pub recommendations: Vec<LearningRecommendation>,
    pub gap_analysis: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Outcome for one installed-skill update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LearningUpdateStatus {
    Updated,
    Unchanged,
    Failed,
}

/// Update outcome for one installed skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LearningUpdateEntry {
    pub name: String,
    pub status: LearningUpdateStatus,
}

/// Summary of registry-backed installed-skill updates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LearnUpdateResult {
    pub success: bool,
    pub updated: u64,
    pub unchanged: u64,
    pub results: Vec<LearningUpdateEntry>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Installation scope for a generated skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillGenerationScope {
    Project,
    User,
}

/// Settings for project-observation skill generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LearnGenerateParams {
    pub scope: SkillGenerationScope,
}

/// Generated skill details.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LearnGenerateResult {
    pub success: bool,
    #[serde(default)]
    pub skill_name: Option<String>,
    #[serde(default)]
    pub skill_path: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Origin of a registered tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolRegistrySource {
    Builtin,
    Meta,
    Extension,
}

/// Persistence scope of a custom registered tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolRegistryScope {
    User,
    Project,
}

/// Built-in, metadata, or extension tool exposed by the CLI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRegistryEntry {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub requires_approval: Option<bool>,
    #[serde(default)]
    pub approval_message: Option<String>,
    pub source: ToolRegistrySource,
    #[serde(default)]
    pub scope: Option<ToolRegistryScope>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub schema_version: Option<u64>,
    #[serde(default)]
    pub handler_preview: Option<String>,
    #[serde(default)]
    pub reuse_hint: Option<String>,
    #[serde(default)]
    pub extension_id: Option<String>,
    #[serde(default)]
    pub extension_version: Option<String>,
}

/// Invalid tool definition skipped by the CLI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRegistryDiagnostic {
    pub file: String,
    pub reason: String,
}

/// Live tool registry and definition diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetToolsRegistryResult {
    pub tools: Vec<ToolRegistryEntry>,
    pub diagnostics: Vec<ToolRegistryDiagnostic>,
}

/// Effective automatic context-compaction setting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextCompactResult {
    pub enabled: bool,
}

/// One completed auto-mode iteration notification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomodeIterationEvent {
    pub session_id: String,
    pub iteration: u64,
    pub actions: Vec<String>,
    #[serde(default)]
    pub tokens_used: Option<u64>,
    pub timestamp: String,
}

/// Terminal auto-mode completion totals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomodeCompleteEvent {
    pub session_id: String,
    pub iterations: u64,
    pub files_created: u64,
    pub files_modified: u64,
    pub timestamp: String,
}

/// Terminal auto-mode failure notification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomodeErrorEvent {
    pub session_id: String,
    pub error: String,
    pub timestamp: String,
}

/// Hook notification emitted immediately before a tool begins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HookPreToolEvent {
    pub tool_id: String,
    pub tool_name: String,
    pub args: serde_json::Map<String, Value>,
    pub timestamp: String,
}
