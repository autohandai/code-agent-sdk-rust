use serde_json::Value;

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
}

pub(crate) fn event_from_notification(method: &str, params: Value) -> SdkEvent {
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
        "autohand.error" => "error",
        _ => method.strip_prefix("autohand.").unwrap_or(method),
    }
    .to_string()
}
