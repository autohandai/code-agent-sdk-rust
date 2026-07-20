use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{
    json_output::{json_instruction, parse_json_as},
    AutohandSdk, AutomodePauseResult, AutomodeResumeResult, AutomodeStartParams,
    AutomodeStartResult, AutomodeStatusResult, AutoresearchCompareParams,
    AutoresearchCompareResult, AutoresearchHistoryResult, AutoresearchParetoResult,
    AutoresearchPinParams, AutoresearchPinResult, AutoresearchPruneParams, AutoresearchPruneResult,
    AutoresearchReplayParams, AutoresearchReplayResult, AutoresearchRescoreParams,
    AutoresearchRescoreResult, AutoresearchStartParams, AutoresearchStartResult,
    AutoresearchStatusResult, AutoresearchStopResult, BrowserHandoffAttachParams,
    BrowserHandoffAttachResult, BrowserHandoffCreateParams, BrowserHandoffCreateResult, Config,
    Error, GetSkillsRegistryParams, GetSkillsRegistryResult, GoalCreateParams, GoalMutationResult,
    GoalSnapshot, GoalTemplateMetadata, GoalUpdateParams, InstallSkillParams, InstallSkillResult,
    McpGetServerConfigsResult, McpListServersResult, McpListToolsParams, McpListToolsResult,
    PromptOptions, ResetResult, Result, SdkEvent,
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

    /// Replaces the active conversation and returns the new session identifier.
    pub async fn reset(&self) -> Result<ResetResult> {
        self.sdk.reset().await
    }

    /// Creates a browser continuation token for the active CLI session.
    pub async fn create_browser_handoff(
        &self,
        params: BrowserHandoffCreateParams,
    ) -> Result<BrowserHandoffCreateResult> {
        self.sdk.create_browser_handoff(params).await
    }

    /// Attaches the session referenced by a browser handoff token.
    pub async fn attach_browser_handoff(
        &self,
        params: BrowserHandoffAttachParams,
    ) -> Result<BrowserHandoffAttachResult> {
        self.sdk.attach_browser_handoff(params).await
    }

    /// Attaches the most recently created, unexpired browser handoff.
    pub async fn attach_latest_browser_handoff(&self) -> Result<BrowserHandoffAttachResult> {
        self.sdk.attach_latest_browser_handoff().await
    }

    /// Starts an auto-mode task and returns once the CLI accepts the session.
    pub async fn start_automode(&self, params: AutomodeStartParams) -> Result<AutomodeStartResult> {
        self.sdk.start_automode(params).await
    }

    /// Returns auto-mode runtime flags and the optional persisted session state.
    pub async fn get_automode_status(&self) -> Result<AutomodeStatusResult> {
        self.sdk.get_automode_status().await
    }

    /// Pauses the active auto-mode session.
    pub async fn pause_automode(&self) -> Result<AutomodePauseResult> {
        self.sdk.pause_automode().await
    }

    /// Resumes a paused auto-mode session.
    pub async fn resume_automode(&self) -> Result<AutomodeResumeResult> {
        self.sdk.resume_automode().await
    }

    /// Starts a high-level slash-command run for an autoresearch objective.
    pub async fn autoresearch(&self, objective: impl Into<String>) -> Result<Run> {
        self.command("/autoresearch", &[objective.into()]).await
    }

    pub async fn command(&self, command: &str, args: &[impl AsRef<str>]) -> Result<Run> {
        let command = crate::format_slash_command(command, args)?;
        self.send(command).await
    }
    pub async fn deep_research(&self, objective: impl Into<String>) -> Result<Run> {
        self.command("/deep-research", &[objective.into()]).await
    }

    pub async fn get_goal(&self) -> Result<GoalSnapshot> {
        self.sdk.get_goal().await
    }
    pub async fn create_goal(&self, p: GoalCreateParams) -> Result<GoalMutationResult> {
        self.sdk.create_goal(p).await
    }
    pub async fn update_goal(&self, p: GoalUpdateParams) -> Result<GoalMutationResult> {
        self.sdk.update_goal(p).await
    }
    pub async fn queue_goal(&self, p: GoalCreateParams) -> Result<GoalMutationResult> {
        self.sdk.queue_goal(p).await
    }
    pub async fn start_queued_goal(&self) -> Result<GoalMutationResult> {
        self.sdk.start_queued_goal().await
    }
    pub async fn list_goal_templates(&self) -> Result<Vec<GoalTemplateMetadata>> {
        self.sdk.list_goal_templates().await
    }
    pub async fn clear_goal(&self) -> Result<GoalMutationResult> {
        self.sdk.clear_goal().await
    }

    pub async fn get_skills_registry(
        &self,
        params: GetSkillsRegistryParams,
    ) -> Result<GetSkillsRegistryResult> {
        self.sdk.get_skills_registry(params).await
    }

    pub async fn install_skill(&self, params: InstallSkillParams) -> Result<InstallSkillResult> {
        self.sdk.install_skill(params).await
    }

    pub async fn list_mcp_servers(&self) -> Result<McpListServersResult> {
        self.sdk.list_mcp_servers().await
    }

    pub async fn list_mcp_tools(&self, params: McpListToolsParams) -> Result<McpListToolsResult> {
        self.sdk.list_mcp_tools(params).await
    }

    pub async fn get_mcp_server_configs(&self) -> Result<McpGetServerConfigsResult> {
        self.sdk.get_mcp_server_configs().await
    }

    /// Initializes or resumes a persisted autoresearch loop.
    pub async fn start_autoresearch(
        &self,
        params: AutoresearchStartParams,
    ) -> Result<AutoresearchStartResult> {
        self.sdk.start_autoresearch(params).await
    }

    /// Returns current persisted autoresearch state.
    pub async fn get_autoresearch_status(&self) -> Result<AutoresearchStatusResult> {
        self.sdk.get_autoresearch_status().await
    }

    /// Pauses autoresearch without deleting persisted state.
    pub async fn stop_autoresearch(&self) -> Result<AutoresearchStopResult> {
        self.sdk.stop_autoresearch().await
    }

    /// Lists persisted autoresearch attempts.
    pub async fn get_autoresearch_history(&self) -> Result<AutoresearchHistoryResult> {
        self.sdk.get_autoresearch_history().await
    }

    /// Re-evaluates a candidate in an isolated worktree.
    pub async fn replay_autoresearch(
        &self,
        params: AutoresearchReplayParams,
    ) -> Result<AutoresearchReplayResult> {
        self.sdk.replay_autoresearch(params).await
    }

    /// Reapplies current decision policy to persisted measurements.
    pub async fn rescore_autoresearch(
        &self,
        params: AutoresearchRescoreParams,
    ) -> Result<AutoresearchRescoreResult> {
        self.sdk.rescore_autoresearch(params).await
    }

    /// Compares persisted evidence for two attempts.
    pub async fn compare_autoresearch(
        &self,
        params: AutoresearchCompareParams,
    ) -> Result<AutoresearchCompareResult> {
        self.sdk.compare_autoresearch(params).await
    }

    /// Returns the current constraint-passing Pareto frontier.
    pub async fn get_autoresearch_pareto(&self) -> Result<AutoresearchParetoResult> {
        self.sdk.get_autoresearch_pareto().await
    }

    /// Pins or unpins a candidate's replay artifacts.
    pub async fn pin_autoresearch(
        &self,
        params: AutoresearchPinParams,
    ) -> Result<AutoresearchPinResult> {
        self.sdk.pin_autoresearch(params).await
    }

    /// Previews or applies artifact retention.
    pub async fn prune_autoresearch(
        &self,
        params: AutoresearchPruneParams,
    ) -> Result<AutoresearchPruneResult> {
        self.sdk.prune_autoresearch(params).await
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
