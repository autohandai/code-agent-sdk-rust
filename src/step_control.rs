use std::{fmt, future::Future, pin::Pin, sync::Arc};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::{Error, Result};

/// A tool call produced during one completed tool step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentStepToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub tool: String,
    pub args: Map<String, Value>,
}

/// A tool result persisted before the host evaluates stop conditions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentStepToolResult {
    pub tool: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Serializable record of one completed tool step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentStep {
    #[serde(deserialize_with = "positive_step_number")]
    pub step_number: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<String>,
    pub tool_calls: Vec<AgentStepToolCall>,
    pub tool_results: Vec<AgentStepToolResult>,
}

fn positive_step_number<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<usize, D::Error> {
    let number = usize::deserialize(deserializer)?;
    if number == 0 {
        return Err(serde::de::Error::custom("stepNumber must be positive"));
    }
    Ok(number)
}

/// Completed step awaiting a host decision when stop control is enabled.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StepEndEvent {
    pub step_id: String,
    pub step: AgentStep,
    pub timestamp: String,
}

/// Ordered completed steps. Shared snapshots avoid copying the history for each predicate.
#[derive(Debug, Clone, Default)]
pub struct StopConditionContext {
    pub steps: Arc<Vec<AgentStep>>,
}

type StopFuture = Pin<Box<dyn Future<Output = Result<bool>> + Send>>;

/// A host callback returning whether the CLI should stop before its next model call.
#[derive(Clone)]
pub struct StopCondition(Arc<dyn Fn(StopConditionContext) -> StopFuture + Send + Sync>);

impl StopCondition {
    /// Construct an asynchronous predicate. Errors settle the turn before surfacing.
    pub fn new<F, Fut>(condition: F) -> Self
    where
        F: Fn(StopConditionContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<bool>> + Send + 'static,
    {
        Self(Arc::new(move |context| Box::pin(condition(context))))
    }

    /// Construct a synchronous predicate for inexpensive local decisions.
    pub fn from_fn<F>(condition: F) -> Self
    where
        F: Fn(&StopConditionContext) -> Result<bool> + Send + Sync + 'static,
    {
        Self::new(move |context| std::future::ready(condition(&context)))
    }

    pub(crate) async fn evaluate(&self, context: StopConditionContext) -> Result<bool> {
        (self.0)(context).await
    }
}

impl fmt::Debug for StopCondition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StopCondition(..)")
    }
}

/// Stop after at least `count` completed tool steps in this prompt.
pub fn is_step_count(count: usize) -> Result<StopCondition> {
    if count == 0 {
        return Err(Error::InvalidInput(
            "step count must be a positive integer".into(),
        ));
    }
    Ok(StopCondition::from_fn(move |context| {
        Ok(context.steps.len() >= count)
    }))
}

/// Stop after a completed step that called the named tool.
pub fn has_tool_call(tool_name: impl Into<String>) -> Result<StopCondition> {
    let tool_name = tool_name.into().trim().to_owned();
    if tool_name.is_empty() {
        return Err(Error::InvalidInput("tool name must be non-empty".into()));
    }
    Ok(StopCondition::from_fn(move |context| {
        Ok(context
            .steps
            .last()
            .is_some_and(|step| step.tool_calls.iter().any(|call| call.tool == tool_name)))
    }))
}
