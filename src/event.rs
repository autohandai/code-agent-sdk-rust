use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnEndUsage {
    pub tokens_used: Option<u64>,
    pub tokens_usage_status: Option<TokenUsageStatus>,
    pub duration_ms: Option<u64>,
    pub context_percent: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TokenUsageStatus {
    Actual,
    Unavailable,
}

use crate::{AutoresearchEvent, AutoresearchLifecycleEvent, AutoresearchOperationEvent};

#[derive(Debug, Clone, PartialEq)]
pub struct SdkEvent {
    pub event_type: String,
    pub raw: Value,
}

impl SdkEvent {
    pub fn new(event_type: impl Into<String>, raw: Value) -> Self {
        Self {
            event_type: event_type.into(),
            raw,
        }
    }

    pub fn text_delta(&self) -> Option<&str> {
        self.raw.get("delta").and_then(Value::as_str)
    }

    pub fn message_content(&self) -> Option<&str> {
        self.raw.get("content").and_then(Value::as_str)
    }

    pub fn tool_name(&self) -> Option<&str> {
        self.raw
            .get("toolName")
            .or_else(|| self.raw.get("tool_name"))
            .and_then(Value::as_str)
    }

    pub fn request_id(&self) -> Option<&str> {
        self.raw
            .get("requestId")
            .or_else(|| self.raw.get("request_id"))
            .and_then(Value::as_str)
    }

    pub fn description(&self) -> Option<&str> {
        self.raw.get("description").and_then(Value::as_str)
    }

    /// Decodes a typed autoresearch lifecycle or ledger-operation event.
    pub fn autoresearch(&self) -> Option<serde_json::Result<AutoresearchEvent>> {
        if self.event_type != "autoresearch" {
            return None;
        }
        if self.raw.get("operation").is_some() {
            return Some(
                serde_json::from_value::<AutoresearchOperationEvent>(self.raw.clone())
                    .map(AutoresearchEvent::Operation),
            );
        }
        Some(
            serde_json::from_value::<AutoresearchLifecycleEvent>(self.raw.clone())
                .map(AutoresearchEvent::Lifecycle),
        )
    }

    pub fn turn_end_usage(&self) -> Option<serde_json::Result<TurnEndUsage>> {
        (self.event_type == "turn_end").then(|| serde_json::from_value(self.raw.clone()))
    }

    /// Decodes an auto-mode iteration while retaining `raw` for forward
    /// compatibility.
    pub fn automode_iteration(&self) -> Option<serde_json::Result<crate::AutomodeIterationEvent>> {
        (self.event_type == "automode_iteration").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn automode_complete(&self) -> Option<serde_json::Result<crate::AutomodeCompleteEvent>> {
        (self.event_type == "automode_complete").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn automode_error(&self) -> Option<serde_json::Result<crate::AutomodeErrorEvent>> {
        (self.event_type == "automode_error").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_pre_tool(&self) -> Option<serde_json::Result<crate::HookPreToolEvent>> {
        (self.event_type == "hook_pre_tool").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_post_tool(&self) -> Option<serde_json::Result<crate::HookPostToolEvent>> {
        (self.event_type == "hook_post_tool").then(|| serde_json::from_value(self.raw.clone()))
    }
}

pub(crate) fn event_from_notification(method: &str, mut params: Value) -> SdkEvent {
    if let Some(object) = params.as_object_mut() {
        match method {
            "autohand.autoresearch.start" => {
                object.insert(
                    "type".to_string(),
                    Value::String("autoresearch".to_string()),
                );
                object.insert("phase".to_string(), Value::String("start".to_string()));
            }
            "autohand.autoresearch.status" => {
                object.insert(
                    "type".to_string(),
                    Value::String("autoresearch".to_string()),
                );
                object.insert("phase".to_string(), Value::String("status".to_string()));
            }
            "autohand.autoresearch.pause" => {
                object.insert(
                    "type".to_string(),
                    Value::String("autoresearch".to_string()),
                );
                object.insert("phase".to_string(), Value::String("pause".to_string()));
            }
            "autohand.autoresearch.event" => {
                object.insert(
                    "type".to_string(),
                    Value::String("autoresearch".to_string()),
                );
            }
            _ => {}
        }
    }
    let event_type = params
        .get("type")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| method_to_type(method));
    SdkEvent::new(event_type, params)
}

fn method_to_type(method: &str) -> String {
    match method {
        "autohand.agentStart" => "agent_start",
        "autohand.agentEnd" => "agent_end",
        "autohand.turnStart" => "turn_start",
        "autohand.turnEnd" => "turn_end",
        "autohand.messageStart" => "message_start",
        "autohand.messageUpdate" => "message_update",
        "autohand.messageEnd" => "message_end",
        "autohand.toolStart" => "tool_start",
        "autohand.toolUpdate" => "tool_update",
        "autohand.toolEnd" => "tool_end",
        "autohand.permissionRequest" => "permission_request",
        "autohand.automode.iteration" => "automode_iteration",
        "autohand.automode.complete" => "automode_complete",
        "autohand.automode.error" => "automode_error",
        "autohand.hook.preTool" => "hook_pre_tool",
        "autohand.hook.postTool" => "hook_post_tool",
        "autohand.autoresearch.start"
        | "autohand.autoresearch.status"
        | "autohand.autoresearch.pause"
        | "autohand.autoresearch.event" => "autoresearch",
        "autohand.error" => "error",
        _ => method.strip_prefix("autohand.").unwrap_or(method),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{AutoresearchLifecyclePhase, AutoresearchOperation, AutoresearchOperationPhase};

    #[test]
    fn maps_autoresearch_lifecycle_notification() {
        let event = event_from_notification(
            "autohand.autoresearch.status",
            json!({
                "active": true,
                "goal": "Reduce latency",
                "iteration": 2,
                "maxIterations": 8,
                "runsLogged": 3,
                "statusText": "Auto-research active",
                "subcommand": "status",
                "timestamp": "2026-07-17T00:00:00Z"
            }),
        );
        assert_eq!(event.event_type, "autoresearch");
        let decoded = event
            .autoresearch()
            .expect("autoresearch event")
            .expect("valid lifecycle event");
        match decoded {
            AutoresearchEvent::Lifecycle(lifecycle) => {
                assert_eq!(lifecycle.phase, AutoresearchLifecyclePhase::Status);
                assert_eq!(lifecycle.runs_logged, 3);
            }
            AutoresearchEvent::Operation(_) => panic!("expected lifecycle event"),
        }
    }

    #[test]
    fn maps_autoresearch_operation_notification() {
        let event = event_from_notification(
            "autohand.autoresearch.event",
            json!({
                "operation": "replay",
                "phase": "completed",
                "attemptId": "attempt-1",
                "success": true,
                "timestamp": "2026-07-17T00:00:01Z"
            }),
        );
        let decoded = event
            .autoresearch()
            .expect("autoresearch event")
            .expect("valid operation event");
        match decoded {
            AutoresearchEvent::Operation(operation) => {
                assert_eq!(operation.operation, AutoresearchOperation::Replay);
                assert_eq!(operation.phase, AutoresearchOperationPhase::Completed);
            }
            AutoresearchEvent::Lifecycle(_) => panic!("expected operation event"),
        }
    }
}
