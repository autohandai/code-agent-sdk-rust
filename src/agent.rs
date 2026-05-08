use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{
    json_output::{json_instruction, parse_json_as},
    AutohandSdk, Config, Error, PromptOptions, Result, SdkEvent,
};

#[derive(Debug, Clone, Default)]
pub struct JsonRunOptions {
    pub schema_name: Option<String>,
    pub schema: Option<Value>,
    pub output_instructions: Option<String>,
}

pub struct Agent {
    sdk: AutohandSdk,
}

impl Agent {
    pub async fn create(config: Config) -> Result<Self> {
        let mut sdk = AutohandSdk::new(config);
        sdk.start().await?;
        Ok(Self { sdk })
    }

    pub fn from_sdk(sdk: AutohandSdk) -> Self {
        Self { sdk }
    }

    pub async fn send(&self, prompt: impl Into<String>) -> Result<Run> {
        let events = self
            .sdk
            .stream_prompt(prompt.into(), PromptOptions::default())
            .await?;
        Ok(Run::new(events, self.sdk.clone()))
    }

    pub async fn run(&self, prompt: impl Into<String>) -> Result<RunResult> {
        let mut run = self.send(prompt).await?;
        run.wait().await
    }

    pub async fn run_json<T: DeserializeOwned>(
        &self,
        prompt: impl Into<String>,
        options: JsonRunOptions,
    ) -> Result<T> {
        let prompt = format!(
            "{}\n\n{}",
            prompt.into(),
            json_instruction(
                options.schema_name.as_deref(),
                options.schema.as_ref(),
                options.output_instructions.as_deref()
            )
        );
        let result = self.run(prompt).await?;
        parse_json_as(&result.text)
    }

    pub async fn allow_permission(&self, request_id: impl Into<String>) -> Result<Value> {
        self.sdk.permission_response(request_id, "allow_once").await
    }

    pub async fn deny_permission(&self, request_id: impl Into<String>) -> Result<Value> {
        self.sdk.permission_response(request_id, "deny_once").await
    }

    pub async fn set_plan_mode(&self, enabled: bool) -> Result<Value> {
        self.sdk.set_plan_mode(enabled).await
    }

    pub async fn close(&mut self) -> Result<()> {
        self.sdk.stop().await
    }
}

pub struct Run {
    id: String,
    events: tokio::sync::mpsc::Receiver<Result<SdkEvent>>,
    sdk: AutohandSdk,
    seen: Vec<SdkEvent>,
    text: String,
}

impl Run {
    fn new(events: tokio::sync::mpsc::Receiver<Result<SdkEvent>>, sdk: AutohandSdk) -> Self {
        Self {
            id: format!(
                "run_{}_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or_default(),
                std::process::id()
            ),
            events,
            sdk,
            seen: Vec::new(),
            text: String::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn next(&mut self) -> Option<Result<SdkEvent>> {
        match self.events.recv().await {
            Some(Ok(event)) => {
                self.record(&event);
                Some(Ok(event))
            }
            Some(Err(error)) => Some(Err(error)),
            None => None,
        }
    }

    pub async fn wait(&mut self) -> Result<RunResult> {
        while let Some(event) = self.next().await {
            event?;
        }
        Ok(RunResult {
            id: self.id.clone(),
            status: "completed".to_string(),
            text: self.text.clone(),
            events: self.seen.clone(),
        })
    }

    pub async fn json<T: DeserializeOwned>(&mut self) -> Result<T> {
        let result = self.wait().await?;
        parse_json_as(&result.text)
    }

    pub async fn abort(&self) -> Result<()> {
        self.sdk.interrupt().await.map(|_| ()).or_else(|error| {
            if matches!(error, Error::TransportNotStarted) {
                Ok(())
            } else {
                Err(error)
            }
        })
    }

    fn record(&mut self, event: &SdkEvent) {
        if let Some(delta) = event.text_delta() {
            self.text.push_str(delta);
        }
        if let Some(content) = event.message_content() {
            self.text.clear();
            self.text.push_str(content);
        }
        self.seen.push(event.clone());
    }
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub id: String,
    pub status: String,
    pub text: String,
    pub events: Vec<SdkEvent>,
}
