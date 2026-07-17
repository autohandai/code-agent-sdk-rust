use std::collections::BTreeMap;

use serde::{ser::SerializeMap, Deserialize, Serialize, Serializer};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoresearchOptimizationDirection {
    Lower,
    Higher,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchSubagentOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idea_generation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_analysis: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finalization: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchSecondaryObjective {
    pub name: String,
    pub unit: String,
    pub direction: AutoresearchOptimizationDirection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchConstraint {
    pub metric_name: String,
    pub operator: AutoresearchConstraintOperator,
    pub threshold: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoresearchConstraintOperator {
    #[serde(rename = "<")]
    LessThan,
    #[serde(rename = "<=")]
    LessThanOrEqual,
    #[serde(rename = ">")]
    GreaterThan,
    #[serde(rename = ">=")]
    GreaterThanOrEqual,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchSamplingOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_samples: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_samples: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchRetentionOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_artifact_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_artifact_age_days: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchStartParams {
    pub objective: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<AutoresearchOptimizationDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measure_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measure_script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks_script: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_in_scope: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagents: Option<AutoresearchSubagentOptions>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary_objectives: Vec<AutoresearchSecondaryObjective>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<AutoresearchConstraint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<AutoresearchSamplingOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention: Option<AutoresearchRetentionOptions>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchMetricAggregate {
    pub median: f64,
    pub mad: f64,
    pub sample_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchEvaluationSample {
    pub sequence: u32,
    pub metrics: BTreeMap<String, f64>,
    pub output_object: String,
    pub duration_ms: u64,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchChecksResult {
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_object: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoresearchExecutionOutcome {
    Passed,
    BenchmarkFailed,
    ChecksFailed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchExecutionResult {
    pub outcome: AutoresearchExecutionOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_object: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoresearchEvaluatorMode {
    Original,
    Current,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchEvaluationRecord {
    pub schema_version: u32,
    #[serde(rename = "type")]
    pub record_type: String,
    pub id: String,
    pub attempt_id: String,
    pub timestamp: String,
    pub context: BTreeMap<String, Value>,
    pub evaluator_mode: AutoresearchEvaluatorMode,
    pub samples: Vec<AutoresearchEvaluationSample>,
    pub aggregates: BTreeMap<String, AutoresearchMetricAggregate>,
    pub checks: AutoresearchChecksResult,
    pub execution: AutoresearchExecutionResult,
    pub drift_warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchConstraintResult {
    pub metric_name: String,
    pub operator: AutoresearchConstraintOperator,
    pub threshold: f64,
    pub conservative_value: f64,
    pub passed: bool,
    pub conclusive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoresearchDecisionSource {
    Original,
    Replay,
    Rescore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoresearchDecisionOutcome {
    Accepted,
    Rejected,
    Inconclusive,
    ChecksFailed,
    Crashed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchDecisionRecord {
    pub schema_version: u32,
    #[serde(rename = "type")]
    pub record_type: String,
    pub id: String,
    pub attempt_id: String,
    pub timestamp: String,
    pub context: BTreeMap<String, Value>,
    pub policy_version: String,
    pub evaluation_id: String,
    pub source: AutoresearchDecisionSource,
    pub constraint_results: Vec<AutoresearchConstraintResult>,
    pub primary_improvement: f64,
    pub confidence: f64,
    pub outcome: AutoresearchDecisionOutcome,
    pub materialized: bool,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoresearchMaterializationState {
    Baseline,
    Committed,
    Retained,
    Reverted,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchHistoryAttempt {
    pub attempt_id: String,
    pub description: String,
    pub timestamp: String,
    pub legacy: bool,
    pub replayable: bool,
    pub pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_evaluation: Option<AutoresearchEvaluationRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_decision: Option<AutoresearchDecisionRecord>,
    pub materialization: AutoresearchMaterializationState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchState {
    pub active: bool,
    pub goal: String,
    pub iteration: u32,
    pub max_iterations: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchStartResult {
    pub success: bool,
    pub message: Option<String>,
    pub instruction: Option<String>,
    pub active: Option<bool>,
    pub state: Option<AutoresearchState>,
    pub status_text: Option<String>,
    pub runs_logged: Option<u32>,
    pub attempts: Option<Vec<AutoresearchHistoryAttempt>>,
    pub pareto_attempt_ids: Option<Vec<String>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchStatusResult {
    pub success: bool,
    pub active: bool,
    pub state: Option<AutoresearchState>,
    pub status_text: String,
    pub runs_logged: u32,
    pub attempts: Option<Vec<AutoresearchHistoryAttempt>>,
    pub pareto_attempt_ids: Option<Vec<String>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchStopResult {
    pub success: bool,
    pub message: Option<String>,
    pub active: Option<bool>,
    pub state: Option<AutoresearchState>,
    pub status_text: Option<String>,
    pub runs_logged: Option<u32>,
    pub attempts: Option<Vec<AutoresearchHistoryAttempt>>,
    pub pareto_attempt_ids: Option<Vec<String>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchHistoryResult {
    pub success: bool,
    pub attempts: Vec<AutoresearchHistoryAttempt>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchReplayParams {
    pub attempt_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluator: Option<AutoresearchEvaluatorMode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchReplayResult {
    pub success: bool,
    pub attempt_id: Option<String>,
    pub evaluator_mode: Option<AutoresearchEvaluatorMode>,
    pub metrics: Option<BTreeMap<String, f64>>,
    pub samples: Option<Vec<AutoresearchEvaluationSample>>,
    pub decision: Option<AutoresearchDecisionRecord>,
    pub drift_warnings: Option<Vec<String>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoresearchRescoreParams {
    Attempt { attempt_id: String },
    All,
}

impl AutoresearchRescoreParams {
    pub fn attempt(attempt_id: impl Into<String>) -> Self {
        Self::Attempt {
            attempt_id: attempt_id.into(),
        }
    }

    pub fn all() -> Self {
        Self::All
    }
}

impl Serialize for AutoresearchRescoreParams {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Self::Attempt { attempt_id } => map.serialize_entry("attemptId", attempt_id)?,
            Self::All => map.serialize_entry("all", &true)?,
        }
        map.end()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchRescoreResult {
    pub success: bool,
    pub decisions: Vec<AutoresearchDecisionRecord>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchCompareParams {
    pub left_attempt_id: String,
    pub right_attempt_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchComparisonSide {
    pub attempt_id: String,
    pub samples: Vec<AutoresearchEvaluationSample>,
    pub aggregates: BTreeMap<String, AutoresearchMetricAggregate>,
    pub checks: AutoresearchChecksResult,
    pub execution: AutoresearchExecutionResult,
    pub decision: Option<AutoresearchDecisionRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchComparison {
    pub left: AutoresearchComparisonSide,
    pub right: AutoresearchComparisonSide,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchCompareResult {
    pub success: bool,
    pub comparison: Option<AutoresearchComparison>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchParetoResult {
    pub success: bool,
    pub attempt_ids: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchPinParams {
    pub attempt_id: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchPinResult {
    pub success: bool,
    pub attempt_id: String,
    pub pinned: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchPruneParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yes: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchPruneCandidate {
    pub attempt_id: String,
    pub objects: Vec<String>,
    pub bytes: u64,
    pub protected: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchPruneResult {
    pub success: bool,
    pub applied: bool,
    pub candidates: Vec<AutoresearchPruneCandidate>,
    pub bytes_freed: u64,
    pub remaining_bytes: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoresearchLifecyclePhase {
    Start,
    Status,
    Pause,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchLifecycleEvent {
    #[serde(rename = "type", default = "autoresearch_event_type")]
    pub event_type: String,
    pub phase: AutoresearchLifecyclePhase,
    pub active: bool,
    pub goal: Option<String>,
    pub iteration: Option<u32>,
    pub max_iterations: Option<u32>,
    pub runs_logged: u32,
    pub status_text: String,
    pub subcommand: AutoresearchSubcommand,
    pub message: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoresearchSubcommand {
    Start,
    Resume,
    Status,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoresearchOperation {
    History,
    Replay,
    Rescore,
    Compare,
    Pareto,
    Pin,
    Prune,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoresearchOperationPhase {
    Started,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoresearchOperationEvent {
    #[serde(rename = "type", default = "autoresearch_event_type")]
    pub event_type: String,
    pub operation: AutoresearchOperation,
    pub phase: AutoresearchOperationPhase,
    pub attempt_id: Option<String>,
    pub success: bool,
    pub applied: Option<bool>,
    pub error: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoresearchEvent {
    Lifecycle(AutoresearchLifecycleEvent),
    Operation(AutoresearchOperationEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoresearchHookEvent {
    #[serde(rename = "autoresearch:start")]
    Start,
    #[serde(rename = "autoresearch:pause")]
    Pause,
    #[serde(rename = "autoresearch:init")]
    Init,
    #[serde(rename = "autoresearch:before")]
    Before,
    #[serde(rename = "autoresearch:run")]
    Run,
    #[serde(rename = "autoresearch:after")]
    After,
    #[serde(rename = "autoresearch:log")]
    Log,
    #[serde(rename = "autoresearch:decision")]
    Decision,
    #[serde(rename = "autoresearch:replay")]
    Replay,
    #[serde(rename = "autoresearch:rescore")]
    Rescore,
    #[serde(rename = "autoresearch:prune")]
    Prune,
    #[serde(rename = "autoresearch:complete")]
    Complete,
    #[serde(rename = "autoresearch:error")]
    Error,
}

fn autoresearch_event_type() -> String {
    "autoresearch".to_string()
}
