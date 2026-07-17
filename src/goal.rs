use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GoalStatus {
    Active,
    Paused,
    BudgetLimited,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalState {
    pub goal_id: String,
    pub objective: String,
    pub status: GoalStatus,
    pub token_budget: Option<u64>,
    pub time_budget_seconds: Option<u64>,
    pub min_tokens_before_wrap_up: Option<u64>,
    pub min_time_seconds_before_wrap_up: Option<u64>,
    pub tokens_used: u64,
    pub time_used_seconds: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedGoal {
    pub queue_id: String,
    pub objective: String,
    pub token_budget: Option<u64>,
    pub time_budget_seconds: Option<u64>,
    pub min_tokens_before_wrap_up: Option<u64>,
    pub min_time_seconds_before_wrap_up: Option<u64>,
    pub source: String,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub template_flags: Vec<String>,
    #[serde(default)]
    pub template_args: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedGoal {
    pub goal_id: String,
    pub objective: String,
    pub status: GoalStatus,
    pub tokens_used: u64,
    pub time_used_seconds: u64,
    pub created_at: String,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalSnapshot {
    pub version: u32,
    pub goal: Option<GoalState>,
    #[serde(default)]
    pub queue: Vec<QueuedGoal>,
    #[serde(default)]
    pub completed: Vec<CompletedGoal>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalTemplateMetadata {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub allow_commands: Vec<String>,
    #[serde(default)]
    pub required_placeholders: Vec<String>,
    #[serde(default)]
    pub required_flags: Vec<String>,
    pub requires_args: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoalCreateParams {
    pub objective: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_budget_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_tokens_before_wrap_up: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_time_seconds_before_wrap_up: Option<u64>,
}
impl GoalCreateParams {
    pub fn new(objective: impl Into<String>) -> Self {
        Self {
            objective: objective.into(),
            token_budget: None,
            time_budget_seconds: None,
            min_tokens_before_wrap_up: None,
            min_time_seconds_before_wrap_up: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GoalUpdateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<GoalStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<Option<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_budget_seconds: Option<Option<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_tokens_before_wrap_up: Option<Option<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_time_seconds_before_wrap_up: Option<Option<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalMutationResult {
    pub ok: bool,
    pub goal: Option<GoalState>,
    #[serde(default)]
    pub queue: Vec<QueuedGoal>,
    pub telemetry: Option<GoalTelemetry>,
    pub message: Option<String>,
    pub queued: Option<QueuedGoal>,
    pub started: Option<GoalState>,
    pub completed: Option<CompletedGoal>,
    pub completed_run: Option<CompletedGoal>,
    pub dequeued: Option<QueuedGoal>,
    #[serde(default)]
    pub removed: Vec<QueuedGoal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalTelemetry {
    pub time_remaining_seconds: Option<u64>,
    pub tokens_remaining: Option<u64>,
    pub completion_floor_met: bool,
}

pub const GOAL_WRITTEN_COMPLETED_HOOK: &str = "goal-written:completed";
