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

macro_rules! notification_kinds {
    ($($kind:ident => ($method:literal, $event_type:literal)),+ $(,)?) => {
        /// Canonical CLI notification identity, independent of its payload fields.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum NotificationKind {
            $(#[serde(rename = $method)] $kind,)+
        }

        impl NotificationKind {
            pub const ALL: &'static [Self] = &[$(Self::$kind,)+];

            pub fn from_method(method: &str) -> Option<Self> {
                match method {
                    $($method => Some(Self::$kind),)+
                    _ => None,
                }
            }

            pub fn method(self) -> &'static str {
                match self { $(Self::$kind => $method,)+ }
            }

            pub fn event_type(self) -> &'static str {
                match self { $(Self::$kind => $event_type,)+ }
            }
        }
    };
}

notification_kinds! {
    AgentStart => ("autohand.agentStart", "agent_start"),
    AgentEnd => ("autohand.agentEnd", "agent_end"),
    Ping => ("autohand.ping", "ping"),
    PeerUpdate => ("autohand.peerUpdate", "peer_update"),
    ResourceUpdate => ("autohand.resourceUpdate", "resource_update"),
    TurnStart => ("autohand.turnStart", "turn_start"),
    TurnEnd => ("autohand.turnEnd", "turn_end"),
    MessageStart => ("autohand.messageStart", "message_start"),
    MessageUpdate => ("autohand.messageUpdate", "message_update"),
    MessageEnd => ("autohand.messageEnd", "message_end"),
    ToolStart => ("autohand.toolStart", "tool_start"),
    ToolUpdate => ("autohand.toolUpdate", "tool_update"),
    ToolEnd => ("autohand.toolEnd", "tool_end"),
    StepEnd => ("autohand.stepEnd", "step_end"),
    PermissionRequest => ("autohand.permissionRequest", "permission_request"),
    DirectoryAccessRequest => ("autohand.directoryAccessRequest", "directory_access_request"),
    Error => ("autohand.error", "error"),
    ChangesBatchStart => ("autohand.changesBatchStart", "changes_batch_start"),
    ChangesBatchUpdate => ("autohand.changesBatchUpdate", "changes_batch_update"),
    ChangesBatchEnd => ("autohand.changesBatchEnd", "changes_batch_end"),
    HookProgramStatus => ("autohand.hook.programStatus", "hook_program_status"),
    HookPreTool => ("autohand.hook.preTool", "hook_pre_tool"),
    HookPostTool => ("autohand.hook.postTool", "hook_post_tool"),
    HookFileModified => ("autohand.hook.fileModified", "file_modified"),
    HookPrePrompt => ("autohand.hook.prePrompt", "hook_pre_prompt"),
    HookPostResponse => ("autohand.hook.postResponse", "hook_post_response"),
    HookSessionError => ("autohand.hook.sessionError", "hook_session_error"),
    HookRateLimit => ("autohand.hook.rateLimit", "hook_rate_limit"),
    HookStop => ("autohand.hook.stop", "hook_stop"),
    HookSessionStart => ("autohand.hook.sessionStart", "hook_session_start"),
    HookSessionEnd => ("autohand.hook.sessionEnd", "hook_session_end"),
    HookSubagentStop => ("autohand.hook.subagentStop", "hook_subagent_stop"),
    HookComputerUseStart => ("autohand.hook.computerUseStart", "hook_computer_use_start"),
    HookComputerUseProgress => ("autohand.hook.computerUseProgress", "hook_computer_use_progress"),
    HookComputerUseError => ("autohand.hook.computerUseError", "hook_computer_use_error"),
    HookComputerUseStop => ("autohand.hook.computerUseStop", "hook_computer_use_stop"),
    HookSubagentStart => ("autohand.hook.subagentStart", "hook_subagent_start"),
    HookSubagentProgress => ("autohand.hook.subagentProgress", "hook_subagent_progress"),
    HookSubagentMessage => ("autohand.hook.subagentMessage", "hook_subagent_message"),
    HookSubagentCancelRequested => ("autohand.hook.subagentCancelRequested", "hook_subagent_cancel_requested"),
    HookPermissionRequest => ("autohand.hook.permissionRequest", "hook_permission_request"),
    HookNotification => ("autohand.hook.notification", "hook_notification"),
    HookReviewStart => ("autohand.hook.reviewStart", "hook_review_start"),
    HookReviewEnd => ("autohand.hook.reviewEnd", "hook_review_end"),
    HookReviewPaused => ("autohand.hook.reviewPaused", "hook_review_paused"),
    HookReviewFailed => ("autohand.hook.reviewFailed", "hook_review_failed"),
    HookReviewCompleted => ("autohand.hook.reviewCompleted", "hook_review_completed"),
    AutomodeStart => ("autohand.automode.start", "automode_start"),
    AutomodeIteration => ("autohand.automode.iteration", "automode_iteration"),
    AutomodeCheckpoint => ("autohand.automode.checkpoint", "automode_checkpoint"),
    AutomodePause => ("autohand.automode.pause", "automode_pause"),
    AutomodeResume => ("autohand.automode.resume", "automode_resume"),
    AutomodeCancel => ("autohand.automode.cancel", "automode_cancel"),
    AutomodeComplete => ("autohand.automode.complete", "automode_complete"),
    AutomodeError => ("autohand.automode.error", "automode_error"),
    AutoresearchStart => ("autohand.autoresearch.start", "autoresearch"),
    AutoresearchStatus => ("autohand.autoresearch.status", "autoresearch"),
    AutoresearchPause => ("autohand.autoresearch.pause", "autoresearch"),
    AutoresearchEvent => ("autohand.autoresearch.event", "autoresearch"),
    ModeChange => ("autohand.modeChange", "mode_change"),
    PipeOutput => ("autohand.pipe.output", "pipe_output"),
    PipeComplete => ("autohand.pipe.complete", "pipe_complete"),
    PipeError => ("autohand.pipe.error", "pipe_error"),
    McpInvokeRequest => ("autohand.mcp.invokeRequest", "mcp_invoke_request"),
    McpToolsChanged => ("autohand.mcp.toolsChanged", "mcp_tools_changed"),
    LearnInstallComplete => ("autohand.learn.installComplete", "learn_install_complete"),
    LearnSecurityWarning => ("autohand.learn.securityWarning", "learn_security_warning"),
    LearnProgress => ("autohand.learn.progress", "learn_progress"),
    ScheduleTriggered => ("autohand.schedule.triggered", "schedule_triggered"),
    SetupStarted => ("autohand.setup.started", "setup_started"),
    SetupStepStart => ("autohand.setup.stepStart", "setup_step_start"),
    SetupStepComplete => ("autohand.setup.stepComplete", "setup_step_complete"),
    SetupCancelled => ("autohand.setup.cancelled", "setup_cancelled"),
    SetupError => ("autohand.setup.error", "setup_error"),
    SetupComplete => ("autohand.setup.complete", "setup_complete"),
    HookContextCompacted => ("autohand.hook.contextCompacted", "hook_context_compacted"),
    HookContextOverflow => ("autohand.hook.contextOverflow", "hook_context_overflow"),
    HookContextWarning => ("autohand.hook.contextWarning", "hook_context_warning"),
    HookContextCritical => ("autohand.hook.contextCritical", "hook_context_critical"),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SdkEvent {
    pub event_type: String,
    pub raw: Value,
    /// Original JSON-RPC method, if received from the CLI.
    pub method: Option<String>,
}

impl SdkEvent {
    pub fn new(event_type: impl Into<String>, raw: Value) -> Self {
        Self {
            event_type: event_type.into(),
            raw,
            method: None,
        }
    }

    /// Returns the exact JSON-RPC notification method, including unknown methods.
    pub fn notification_method(&self) -> Option<&str> {
        self.method
            .as_deref()
            .or_else(|| hook_type_to_method(&self.event_type))
    }

    pub fn notification_kind(&self) -> Option<NotificationKind> {
        self.notification_method()
            .and_then(NotificationKind::from_method)
    }

    /// Decodes a notification payload without rewriting the original JSON.
    pub fn decode_payload<T: serde::de::DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_value(self.raw.clone())
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
        let mut payload = self.raw.clone();
        if let Some(object) = payload.as_object_mut() {
            if !object.contains_key("phase") {
                let phase = match self.notification_method() {
                    Some("autohand.autoresearch.start") => Some("start"),
                    Some("autohand.autoresearch.status") => Some("status"),
                    Some("autohand.autoresearch.pause") => Some("pause"),
                    _ => None,
                };
                if let Some(phase) = phase {
                    object.insert("phase".into(), Value::String(phase.into()));
                }
            }
        }
        Some(
            serde_json::from_value::<AutoresearchLifecycleEvent>(payload)
                .map(AutoresearchEvent::Lifecycle),
        )
    }

    /// Decodes a peer/resource coordination update without dropping future event fields.
    pub fn coordination_update(
        &self,
    ) -> Option<serde_json::Result<crate::CoordinationUpdateEvent>> {
        matches!(self.event_type.as_str(), "peer_update" | "resource_update")
            .then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn turn_end_usage(&self) -> Option<serde_json::Result<TurnEndUsage>> {
        (self.event_type == "turn_end").then(|| serde_json::from_value(self.raw.clone()))
    }

    /// Decode a completed tool step while retaining the original event payload.
    pub fn step_end(&self) -> Option<serde_json::Result<crate::StepEndEvent>> {
        (self.event_type == "step_end").then(|| serde_json::from_value(self.raw.clone()))
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

    pub fn hook_pre_prompt(&self) -> Option<serde_json::Result<crate::HookPrePromptEvent>> {
        (self.event_type == "hook_pre_prompt").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_post_response(&self) -> Option<serde_json::Result<crate::HookPostResponseEvent>> {
        (self.event_type == "hook_post_response").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_file_modified(&self) -> Option<serde_json::Result<crate::HookFileModifiedEvent>> {
        (self.event_type == "file_modified").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_session_error(&self) -> Option<serde_json::Result<crate::HookSessionErrorEvent>> {
        (self.event_type == "hook_session_error").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_stop(&self) -> Option<serde_json::Result<crate::HookStopEvent>> {
        (self.event_type == "hook_stop").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_session_start(&self) -> Option<serde_json::Result<crate::HookSessionStartEvent>> {
        (self.event_type == "hook_session_start").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_session_end(&self) -> Option<serde_json::Result<crate::HookSessionEndEvent>> {
        (self.event_type == "hook_session_end").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_subagent_stop(&self) -> Option<serde_json::Result<crate::HookSubagentStopEvent>> {
        (self.event_type == "hook_subagent_stop").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_permission_request(
        &self,
    ) -> Option<serde_json::Result<crate::HookPermissionRequestEvent>> {
        (self.event_type == "hook_permission_request")
            .then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_notification(&self) -> Option<serde_json::Result<crate::HookNotificationEvent>> {
        (self.event_type == "hook_notification").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_context_compacted(
        &self,
    ) -> Option<serde_json::Result<crate::HookContextCompactedEvent>> {
        (self.event_type == "hook_context_compacted")
            .then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_context_overflow(
        &self,
    ) -> Option<serde_json::Result<crate::HookContextOverflowEvent>> {
        (self.event_type == "hook_context_overflow")
            .then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_context_warning(
        &self,
    ) -> Option<serde_json::Result<crate::HookContextWarningEvent>> {
        (self.event_type == "hook_context_warning")
            .then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn hook_context_critical(
        &self,
    ) -> Option<serde_json::Result<crate::HookContextCriticalEvent>> {
        (self.event_type == "hook_context_critical")
            .then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn mcp_invocation_request(
        &self,
    ) -> Option<serde_json::Result<crate::McpInvocationRequestEvent>> {
        (self.event_type == "mcp_invoke_request").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn mcp_tools_changed(&self) -> Option<serde_json::Result<crate::McpToolsChangedEvent>> {
        (self.event_type == "mcp_tools_changed").then(|| serde_json::from_value(self.raw.clone()))
    }

    pub fn learning_progress(&self) -> Option<serde_json::Result<crate::LearningProgressEvent>> {
        (self.event_type == "learn_progress").then(|| serde_json::from_value(self.raw.clone()))
    }
    pub fn directory_access_request(
        &self,
    ) -> Option<serde_json::Result<crate::DirectoryAccessRequestEvent>> {
        matches!(self.event_type.as_str(), "directory_access_request")
            .then(|| self.decode_payload())
    }

    pub fn changes_batch_start(&self) -> Option<serde_json::Result<crate::ChangesBatchStartEvent>> {
        matches!(self.event_type.as_str(), "changes_batch_start").then(|| self.decode_payload())
    }

    pub fn changes_batch_update(
        &self,
    ) -> Option<serde_json::Result<crate::ChangesBatchUpdateEvent>> {
        matches!(self.event_type.as_str(), "changes_batch_update").then(|| self.decode_payload())
    }

    pub fn changes_batch_end(&self) -> Option<serde_json::Result<crate::ChangesBatchEndEvent>> {
        matches!(self.event_type.as_str(), "changes_batch_end").then(|| self.decode_payload())
    }

    pub fn hook_program_status(&self) -> Option<serde_json::Result<crate::HookProgramStatusEvent>> {
        matches!(self.event_type.as_str(), "hook_program_status").then(|| self.decode_payload())
    }

    pub fn hook_rate_limit(&self) -> Option<serde_json::Result<crate::HookRateLimitEvent>> {
        matches!(self.event_type.as_str(), "hook_rate_limit").then(|| self.decode_payload())
    }

    pub fn hook_subagent_activity(
        &self,
    ) -> Option<serde_json::Result<crate::HookSubagentActivityEvent>> {
        matches!(
            self.event_type.as_str(),
            "hook_subagent_start"
                | "hook_subagent_progress"
                | "hook_subagent_message"
                | "hook_subagent_cancel_requested"
        )
        .then(|| self.decode_payload())
    }

    pub fn hook_computer_use(&self) -> Option<serde_json::Result<crate::HookComputerUseEvent>> {
        matches!(
            self.event_type.as_str(),
            "hook_computer_use_start"
                | "hook_computer_use_progress"
                | "hook_computer_use_error"
                | "hook_computer_use_stop"
        )
        .then(|| self.decode_payload())
    }

    pub fn hook_review(&self) -> Option<serde_json::Result<crate::HookReviewEvent>> {
        matches!(
            self.event_type.as_str(),
            "hook_review_start"
                | "hook_review_end"
                | "hook_review_paused"
                | "hook_review_failed"
                | "hook_review_completed"
        )
        .then(|| self.decode_payload())
    }

    pub fn automode_start(&self) -> Option<serde_json::Result<crate::AutomodeStartEvent>> {
        matches!(self.event_type.as_str(), "automode_start").then(|| self.decode_payload())
    }

    pub fn automode_checkpoint(
        &self,
    ) -> Option<serde_json::Result<crate::AutomodeCheckpointEvent>> {
        matches!(self.event_type.as_str(), "automode_checkpoint").then(|| self.decode_payload())
    }

    pub fn automode_pause(&self) -> Option<serde_json::Result<crate::AutomodePauseEvent>> {
        matches!(self.event_type.as_str(), "automode_pause").then(|| self.decode_payload())
    }

    pub fn automode_resume(&self) -> Option<serde_json::Result<crate::AutomodeResumeEvent>> {
        matches!(self.event_type.as_str(), "automode_resume").then(|| self.decode_payload())
    }

    pub fn automode_cancel(&self) -> Option<serde_json::Result<crate::AutomodeCancelEvent>> {
        matches!(self.event_type.as_str(), "automode_cancel").then(|| self.decode_payload())
    }

    pub fn pipe_output(&self) -> Option<serde_json::Result<crate::PipeOutputEvent>> {
        matches!(self.event_type.as_str(), "pipe_output").then(|| self.decode_payload())
    }

    pub fn schedule_triggered(&self) -> Option<serde_json::Result<crate::ScheduleTriggeredEvent>> {
        matches!(self.event_type.as_str(), "schedule_triggered").then(|| self.decode_payload())
    }

    pub fn setup_started(&self) -> Option<serde_json::Result<crate::SetupStartedEvent>> {
        matches!(self.event_type.as_str(), "setup_started").then(|| self.decode_payload())
    }

    pub fn setup_step(&self) -> Option<serde_json::Result<crate::SetupStepEvent>> {
        matches!(
            self.event_type.as_str(),
            "setup_step_start" | "setup_step_complete"
        )
        .then(|| self.decode_payload())
    }

    pub fn setup_cancelled(&self) -> Option<serde_json::Result<crate::SetupCancelledEvent>> {
        matches!(self.event_type.as_str(), "setup_cancelled").then(|| self.decode_payload())
    }

    pub fn setup_error(&self) -> Option<serde_json::Result<crate::SetupErrorEvent>> {
        matches!(self.event_type.as_str(), "setup_error").then(|| self.decode_payload())
    }

    pub fn setup_complete(&self) -> Option<serde_json::Result<crate::SetupCompleteEvent>> {
        matches!(self.event_type.as_str(), "setup_complete").then(|| self.decode_payload())
    }
}

pub(crate) fn event_from_notification(method: &str, params: Value) -> SdkEvent {
    let event_type = NotificationKind::from_method(method)
        .map(|kind| kind.event_type().to_owned())
        .unwrap_or_else(|| {
            method
                .strip_prefix("autohand.")
                .unwrap_or(method)
                .to_owned()
        });
    SdkEvent {
        event_type,
        raw: params,
        method: Some(method.to_owned()),
    }
}

fn hook_type_to_method(event_type: &str) -> Option<&'static str> {
    match event_type {
        "hook_pre_tool" => Some("autohand.hook.preTool"),
        "hook_post_tool" => Some("autohand.hook.postTool"),
        "file_modified" => Some("autohand.hook.fileModified"),
        "hook_pre_prompt" => Some("autohand.hook.prePrompt"),
        "hook_post_response" => Some("autohand.hook.postResponse"),
        "hook_session_error" => Some("autohand.hook.sessionError"),
        "hook_stop" => Some("autohand.hook.stop"),
        "hook_session_start" => Some("autohand.hook.sessionStart"),
        "hook_session_end" => Some("autohand.hook.sessionEnd"),
        "hook_subagent_stop" => Some("autohand.hook.subagentStop"),
        "hook_permission_request" => Some("autohand.hook.permissionRequest"),
        "hook_notification" => Some("autohand.hook.notification"),
        "hook_context_compacted" => Some("autohand.hook.contextCompacted"),
        "hook_context_overflow" => Some("autohand.hook.contextOverflow"),
        "hook_context_warning" => Some("autohand.hook.contextWarning"),
        "hook_context_critical" => Some("autohand.hook.contextCritical"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{AutoresearchLifecyclePhase, AutoresearchOperation, AutoresearchOperationPhase};

    #[test]
    fn every_canonical_method_controls_routing_and_preserves_exact_payload() {
        for kind in NotificationKind::ALL {
            for payload in [
                json!({"type":"spoofed","phase":"wire-phase","nested":{"ok":false}}),
                Value::Null,
                json!([1, 2]),
            ] {
                let event = event_from_notification(kind.method(), payload.clone());
                assert_eq!(event.event_type, kind.event_type());
                assert_eq!(event.notification_kind(), Some(*kind));
                assert_eq!(event.notification_method(), Some(kind.method()));
                assert_eq!(event.raw, payload);
            }
        }
        let event = event_from_notification("autohand.future.event", json!({"type":"agent_end"}));
        assert_eq!(event.event_type, "future.event");
        assert_eq!(event.notification_method(), Some("autohand.future.event"));
        assert_eq!(event.notification_kind(), None);
        assert_eq!(event.raw, json!({"type":"agent_end"}));
    }

    #[test]
    fn typed_current_notifications_decode_without_rewriting_payload_type() {
        let pipe = event_from_notification(
            "autohand.pipe.output",
            json!({"type":"progress","content":"Working","timestamp":"t"}),
        );
        assert_eq!(pipe.event_type, "pipe_output");
        assert_eq!(
            pipe.pipe_output().unwrap().unwrap().output_type,
            crate::PipeOutputType::Progress
        );
        assert_eq!(pipe.raw["type"], "progress");

        let directory = event_from_notification(
            "autohand.directoryAccessRequest",
            json!({"requestId":"dir-1","path":"/workspace","reason":"read fixtures","timestamp":"t"}),
        );
        assert_eq!(
            directory
                .directory_access_request()
                .unwrap()
                .unwrap()
                .request_id,
            "dir-1"
        );
        let status = event_from_notification(
            "autohand.hook.programStatus",
            json!({"state":"blocked","kind":"permission","title":"Allow access","timestamp":"t"}),
        );
        assert!(matches!(
            status.hook_program_status().unwrap().unwrap().status,
            crate::ProgramStatusState::Blocked {
                kind: crate::ProgramStatusKind::Permission,
                ..
            }
        ));
        let rate_limit = event_from_notification(
            "autohand.hook.rateLimit",
            json!({"error":"Too many requests","retryAfterMs":500,"httpStatus":429,"timestamp":"t"}),
        );
        assert_eq!(
            rate_limit.hook_rate_limit().unwrap().unwrap().http_status,
            Some(429)
        );
        let activity = event_from_notification(
            "autohand.hook.subagentMessage",
            json!({"subagentId":"sub-1","subagentName":"reviewer","subagentType":"worker","message":"Found a bug","timestamp":"t"}),
        );
        assert_eq!(
            activity
                .hook_subagent_activity()
                .unwrap()
                .unwrap()
                .message
                .as_deref(),
            Some("Found a bug")
        );
        let computer = event_from_notification(
            "autohand.hook.computerUseStop",
            json!({"runId":"run-1","status":"unverified","success":false,"timestamp":"t"}),
        );
        assert_eq!(
            computer.hook_computer_use().unwrap().unwrap().status,
            Some(crate::ComputerUseStatus::Unverified)
        );
        let malformed = event_from_notification(
            "autohand.hook.computerUseStop",
            json!({"runId":"run-1","status":"invented","timestamp":"t"}),
        );
        assert!(malformed.hook_computer_use().unwrap().is_err());
        let checkpoint = event_from_notification(
            "autohand.automode.checkpoint",
            json!({"sessionId":"auto-1","iteration":4,"commit":"abc123","timestamp":"t"}),
        );
        assert_eq!(
            checkpoint.automode_checkpoint().unwrap().unwrap().iteration,
            4
        );
        let review = event_from_notification(
            "autohand.hook.reviewFailed",
            json!({"event":"review:failed","success":false,"error":"checks failed","timestamp":"t"}),
        );
        assert_eq!(
            review.hook_review().unwrap().unwrap().error.as_deref(),
            Some("checks failed")
        );
        let batch = event_from_notification(
            "autohand.changesBatchEnd",
            json!({"batchId":"batch-1","changeCount":2,"timestamp":"t"}),
        );
        assert_eq!(batch.changes_batch_end().unwrap().unwrap().change_count, 2);
        let setup = event_from_notification(
            "autohand.setup.stepComplete",
            json!({"step":"provider","data":{"provider":"anthropic"},"timestamp":"t"}),
        );
        assert_eq!(
            setup.setup_step().unwrap().unwrap().data.unwrap()["provider"],
            "anthropic"
        );
        let schedule = event_from_notification(
            "autohand.schedule.triggered",
            json!({"scheduleId":"schedule-1","prompt":"Audit dependencies","timestamp":"t"}),
        );
        assert_eq!(
            schedule
                .schedule_triggered()
                .unwrap()
                .unwrap()
                .schedule_id
                .as_deref(),
            Some("schedule-1")
        );
    }

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
        assert!(event.raw.get("phase").is_none());
        assert!(event.raw.get("type").is_none());
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

#[cfg(test)]
mod collaboration_notification_tests {
    use super::*;
    #[test]
    fn coordination_events_have_canonical_identity() {
        for (method, kind) in [
            ("autohand.peerUpdate", "peer_update"),
            ("autohand.resourceUpdate", "resource_update"),
        ] {
            assert_eq!(
                NotificationKind::from_method(method).map(NotificationKind::event_type),
                Some(kind)
            );
        }
    }
}
