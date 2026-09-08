use std::{
    collections::HashMap,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
        Arc, Mutex as StdMutex, Weak,
    },
    time::Duration,
};

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{broadcast, mpsc, oneshot, Mutex, Notify},
    task::JoinSet,
    time,
};

use crate::{
    blueprint::{decode_answer, AnswerRpcResult},
    config::PromptOptions,
    event::event_from_notification,
    AutomodeCancelParams, AutomodeCancelResult, AutomodeGetLogParams, AutomodeGetLogResult,
    AutomodePauseResult, AutomodeResumeResult, AutomodeStartParams, AutomodeStartResult,
    AutomodeStatusResult, AutoresearchCompareParams, AutoresearchCompareResult,
    AutoresearchHistoryResult, AutoresearchParetoResult, AutoresearchPinParams,
    AutoresearchPinResult, AutoresearchPruneParams, AutoresearchPruneResult,
    AutoresearchReplayParams, AutoresearchReplayResult, AutoresearchRescoreParams,
    AutoresearchRescoreResult, AutoresearchStartParams, AutoresearchStartResult,
    AutoresearchStatusResult, AutoresearchStopResult, BrowserHandoffAttachParams,
    BrowserHandoffAttachResult, BrowserHandoffCreateParams, BrowserHandoffCreateResult,
    ClassifiedAnswerEnvelope, Config, Error, GetSkillsRegistryParams, GetSkillsRegistryResult,
    GoalCreateParams, GoalMutationResult, GoalSnapshot, GoalTemplateMetadata, GoalUpdateParams,
    InstallSkillParams, InstallSkillResult, LoginChallenge, LoginChallengeWire, LoginProblem,
    LoginProblemCode, LoginSession, LoginStatus, LoginStatusWire, McpGetServerConfigsResult,
    McpListServersResult, McpListToolsParams, McpListToolsResult, ResetResult, Result,
    RuntimeFacts, RuntimeProfile, SdkEvent, StepEndEvent, StopCondition, StopConditionContext,
    StrictJsonSchema, StructuredAnswerRun, StructuredRunLimits, AUTOHAND_LOGIN_CONTRACT_VERSION,
};

#[derive(Clone)]
pub struct AutohandSdk {
    config: Config,
    lifecycle: Arc<Lifecycle>,
}

#[derive(Default)]
struct Lifecycle {
    inner: StdMutex<Option<Arc<TransportInner>>>,
}

impl AutohandSdk {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            lifecycle: Arc::new(Lifecycle::default()),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.is_started() {
            return Ok(());
        }

        self.config.validate().map_err(Error::InvalidInput)?;
        let inner = TransportInner::start(self.config.clone()).await?;
        let initialize = match self.config.runtime_profile {
            RuntimeProfile::Interactive => {
                async {
                    inner.request("autohand.getState", json!({})).await?;
                    if let Some(features) = &self.config.features {
                        inner
                            .request(
                                "autohand.applyFlagSettings",
                                json!({"settings":{"features":features}}),
                            )
                            .await?;
                    }
                    Ok(())
                }
                .await
            }
            RuntimeProfile::AnswerOnly(_) => {
                async {
                    let value = inner.request("autohand.runtimeInspect", json!({})).await?;
                    let facts: RuntimeFacts = serde_json::from_value(value)?;
                    facts.validate_answer_only()
                }
                .await
            }
            RuntimeProfile::SetupOnly(_) => Ok(()),
        };
        if let Err(error) = initialize {
            let _ = inner.stop().await;
            return Err(error);
        }

        let mut lifecycle = self
            .lifecycle
            .inner
            .lock()
            .map_err(|_| Error::LifecyclePoisoned)?;
        *lifecycle = Some(inner);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        let inner = self
            .lifecycle
            .inner
            .lock()
            .map_err(|_| Error::LifecyclePoisoned)?
            .take();
        if let Some(inner) = inner {
            inner.stop().await?;
        }
        Ok(())
    }

    pub(crate) fn abort_process_tree_now(&self) {
        if let Ok(mut lifecycle) = self.lifecycle.inner.lock() {
            if let Some(inner) = lifecycle.take() {
                inner.process_tree.terminate_now();
                inner.fail_pending();
            }
        }
    }

    pub(crate) fn event_limits(&self) -> (usize, usize) {
        (self.config.max_events, self.config.max_event_bytes)
    }

    fn validate_login_session(&self, session: &LoginSession) -> Result<()> {
        if !matches!(self.config.runtime_profile, RuntimeProfile::SetupOnly(_))
            || !Arc::ptr_eq(&self.lifecycle, &session.sdk.lifecycle)
        {
            return Err(Error::InvalidInput(
                "login session belongs to a different setup-only SDK process".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn is_started(&self) -> bool {
        self.lifecycle
            .inner
            .lock()
            .map(|inner| inner.is_some())
            .unwrap_or(false)
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        if !rpc_method_allowed(self.config.runtime_profile, method) {
            return Err(Error::ProfileViolation {
                profile: profile_name(self.config.runtime_profile),
                method: method.to_owned(),
            });
        }
        self.inner()?.request(method, params).await
    }

    pub async fn runtime_facts(&self) -> Result<RuntimeFacts> {
        let facts: RuntimeFacts = self
            .request_typed("autohand.runtimeInspect", json!({}))
            .await?;
        if matches!(self.config.runtime_profile, RuntimeProfile::AnswerOnly(_)) {
            facts.validate_answer_only()?;
        }
        Ok(facts)
    }

    pub async fn run_answer<T: DeserializeOwned>(
        &self,
        envelope: ClassifiedAnswerEnvelope,
        schema: StrictJsonSchema,
        limits: StructuredRunLimits,
    ) -> Result<StructuredAnswerRun<T>> {
        if !matches!(self.config.runtime_profile, RuntimeProfile::AnswerOnly(_)) {
            return Err(Error::ProfileViolation {
                profile: profile_name(self.config.runtime_profile),
                method: "autohand.answer".to_owned(),
            });
        }
        limits.validate()?;
        envelope.validate()?;
        if envelope.output_schema != *schema.as_value() {
            return Err(Error::InvalidInput(
                "envelope outputSchema does not match the supplied strict schema".to_owned(),
            ));
        }
        let encoded = serde_json::to_vec(&envelope)?;
        if encoded.len() > limits.max_input_bytes {
            return Err(Error::OutputLimitExceeded {
                stream: "classified answer input",
                limit: limits.max_input_bytes,
            });
        }
        let runtime_facts = self.runtime_facts().await?;
        runtime_facts.validate_answer_destination()?;
        let response: AnswerRpcResult = self.request_typed("autohand.answer", envelope).await?;
        decode_answer(response, runtime_facts, &schema, limits)
    }

    pub async fn begin_autohand_login(&self) -> Result<LoginChallenge> {
        if !matches!(self.config.runtime_profile, RuntimeProfile::SetupOnly(_)) {
            return Err(Error::ProfileViolation {
                profile: profile_name(self.config.runtime_profile),
                method: "autohand.login.begin".to_owned(),
            });
        }
        let challenge: LoginChallengeWire = self
            .request_typed(
                "autohand.login.begin",
                json!({
                    "contractVersion": AUTOHAND_LOGIN_CONTRACT_VERSION,
                    "trafficClass": "autohand_device_authorization",
                }),
            )
            .await?;
        challenge.validate(self.clone())
    }

    pub async fn poll_autohand_login(&self, session: &LoginSession) -> Result<LoginStatus> {
        self.validate_login_session(session)?;
        let status: LoginStatusWire = self
            .request_typed(
                "autohand.login.poll",
                json!({
                    "contractVersion": AUTOHAND_LOGIN_CONTRACT_VERSION,
                    "sessionId": session.id,
                }),
            )
            .await?;
        let status = status.validate()?;
        if !matches!(status, LoginStatus::Pending { .. }) {
            session.finish();
        }
        Ok(status)
    }

    pub async fn cancel_autohand_login(&self, session: LoginSession) -> Result<()> {
        self.validate_login_session(&session)?;
        let status: LoginStatusWire = self
            .request_typed(
                "autohand.login.cancel",
                json!({
                    "contractVersion": AUTOHAND_LOGIN_CONTRACT_VERSION,
                    "sessionId": session.id,
                }),
            )
            .await?;
        match status.validate()? {
            LoginStatus::Cancelled => {
                session.finish();
                Ok(())
            }
            LoginStatus::Failed { problem } => Err(Error::LoginFailed { problem }),
            _ => Err(Error::Protocol(
                "login cancellation did not return terminal cancelled status".to_owned(),
            )),
        }
    }

    pub async fn prompt(
        &self,
        message: impl Into<String>,
        options: PromptOptions,
    ) -> Result<Value> {
        let mut prompt = self.start_prompt(message.into(), options)?;
        while let Some(event) = prompt.events.recv().await {
            event?;
        }
        prompt
            .response
            .await
            .map_err(|_| Error::MissingTerminalEvent)
    }

    pub async fn stream_command(
        &self,
        command: &str,
        args: &[impl AsRef<str>],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<SdkEvent>>> {
        let command = crate::format_slash_command(command, args)?;
        self.stream_prompt(command, PromptOptions::default()).await
    }

    pub async fn supported_commands(&self) -> Result<Vec<String>> {
        let value = self
            .request("autohand.getSupportedCommands", json!({}))
            .await?;
        let commands = value
            .get("commands")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(commands
            .into_iter()
            .filter_map(|v| {
                v.as_str().map(|s| {
                    if s.starts_with('/') {
                        s.to_owned()
                    } else {
                        format!("/{s}")
                    }
                })
            })
            .collect())
    }
    pub async fn supports_command(&self, command: &str) -> Result<bool> {
        let normalized = format!("/{}", command.trim().trim_start_matches('/'));
        Ok(self.supported_commands().await?.contains(&normalized))
    }

    pub async fn get_goal(&self) -> Result<GoalSnapshot> {
        self.request_typed("autohand.goal.get", json!({})).await
    }
    pub async fn create_goal(&self, p: GoalCreateParams) -> Result<GoalMutationResult> {
        self.request_typed("autohand.goal.create", p).await
    }
    pub async fn update_goal(&self, p: GoalUpdateParams) -> Result<GoalMutationResult> {
        self.request_typed("autohand.goal.update", p).await
    }
    pub async fn queue_goal(&self, p: GoalCreateParams) -> Result<GoalMutationResult> {
        self.request_typed("autohand.goal.queue", p).await
    }
    pub async fn start_queued_goal(&self) -> Result<GoalMutationResult> {
        self.request_typed("autohand.goal.startQueued", json!({}))
            .await
    }
    pub async fn list_goal_templates(&self) -> Result<Vec<GoalTemplateMetadata>> {
        self.request_typed("autohand.goal.listTemplates", json!({}))
            .await
    }
    pub async fn clear_goal(&self) -> Result<GoalMutationResult> {
        self.request_typed("autohand.goal.clear", json!({})).await
    }

    pub async fn stream_prompt(
        &self,
        message: impl Into<String>,
        options: PromptOptions,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<SdkEvent>>> {
        self.start_prompt(message.into(), options)
            .map(|prompt| prompt.events)
    }

    pub(crate) fn start_prompt(
        &self,
        message: String,
        options: PromptOptions,
    ) -> Result<PromptStream> {
        if !rpc_method_allowed(self.config.runtime_profile, "autohand.prompt") {
            return Err(Error::ProfileViolation {
                profile: profile_name(self.config.runtime_profile),
                method: "autohand.prompt".to_owned(),
            });
        }
        let inner = self.inner()?.clone();
        let (tx, rx) = mpsc::channel(256);
        let (response_tx, response) = oneshot::channel();
        let params = options.to_params(message);
        let reserved_turn = inner.prompt_lock.clone().try_lock_owned().ok();
        let control = Arc::new(PromptControl {
            state: AtomicU8::new(if reserved_turn.is_some() {
                PROMPT_ACTIVE
            } else {
                PROMPT_QUEUED
            }),
            cancelled: Notify::new(),
            inner: Arc::downgrade(&inner),
            lifecycle: Arc::downgrade(&self.lifecycle),
        });
        let task_control = control.clone();
        tokio::spawn(async move {
            let _turn = match reserved_turn {
                Some(guard) => guard,
                None => {
                    let guard = tokio::select! {
                        biased;
                        _ = tx.closed() => return,
                        _ = task_control.cancelled.notified() => return,
                        guard = inner.prompt_lock.clone().lock_owned() => guard,
                    };
                    if task_control
                        .state
                        .compare_exchange(
                            PROMPT_QUEUED,
                            PROMPT_ACTIVE,
                            Ordering::SeqCst,
                            Ordering::SeqCst,
                        )
                        .is_err()
                    {
                        return;
                    }
                    guard
                }
            };
            // Subscribe only after the previous turn's cleanup has finished.
            let mut events = inner.events.subscribe();
            let (result, settled) = inner
                .pump_prompt(
                    &tx,
                    &task_control,
                    &mut events,
                    params,
                    options.stop_when,
                    response_tx,
                )
                .await;
            if !settled && !task_control.is_cancelled() {
                inner.settle_prompt(&mut events).await;
            }
            task_control.finish();
            drop(_turn);
            if let Err(error) = result {
                let _ = tx.send(Err(error)).await;
            }
        });
        Ok(PromptStream {
            events: rx,
            control,
            response,
        })
    }

    pub async fn interrupt(&self) -> Result<Value> {
        self.request("autohand.abort", json!({})).await
    }

    /// Replaces the active conversation and returns the new session identifier.
    pub async fn reset(&self) -> Result<ResetResult> {
        self.request_typed("autohand.reset", json!({})).await
    }

    pub async fn set_plan_mode(&self, enabled: bool) -> Result<Value> {
        self.request("autohand.planModeSet", json!({ "enabled": enabled }))
            .await
    }

    pub async fn set_permission_mode(&self, mode: impl Into<String>) -> Result<Value> {
        self.request("autohand.permissionModeSet", json!({ "mode": mode.into() }))
            .await
    }

    pub async fn set_model(&self, model: impl Into<String>) -> Result<Value> {
        self.request("autohand.modelSet", json!({ "model": model.into() }))
            .await
    }

    pub async fn get_state(&self) -> Result<Value> {
        self.request("autohand.getState", json!({})).await
    }

    pub async fn get_messages(&self) -> Result<Value> {
        self.request("autohand.getMessages", json!({})).await
    }

    /// Creates a browser continuation token for the active CLI session.
    pub async fn create_browser_handoff(
        &self,
        params: BrowserHandoffCreateParams,
    ) -> Result<BrowserHandoffCreateResult> {
        self.request_typed("autohand.browserHandoff.create", params)
            .await
    }

    /// Attaches the session referenced by a browser handoff token.
    pub async fn attach_browser_handoff(
        &self,
        params: BrowserHandoffAttachParams,
    ) -> Result<BrowserHandoffAttachResult> {
        self.request_typed("autohand.browserHandoff.attach", params)
            .await
    }

    /// Attaches the most recently created, unexpired browser handoff.
    pub async fn attach_latest_browser_handoff(&self) -> Result<BrowserHandoffAttachResult> {
        self.request_typed("autohand.browserHandoff.attachLatest", json!({}))
            .await
    }

    /// Starts an auto-mode task and returns once the CLI accepts the session.
    pub async fn start_automode(&self, params: AutomodeStartParams) -> Result<AutomodeStartResult> {
        if params.prompt.trim().is_empty() {
            return Err(Error::InvalidInput("automode prompt is required".into()));
        }
        self.request_typed("autohand.automode.start", params).await
    }

    /// Returns auto-mode runtime flags and the optional persisted session state.
    pub async fn get_automode_status(&self) -> Result<AutomodeStatusResult> {
        self.request_typed("autohand.automode.status", json!({}))
            .await
    }

    /// Pauses the active auto-mode session.
    pub async fn pause_automode(&self) -> Result<AutomodePauseResult> {
        self.request_typed("autohand.automode.pause", json!({}))
            .await
    }

    /// Resumes a paused auto-mode session.
    pub async fn resume_automode(&self) -> Result<AutomodeResumeResult> {
        self.request_typed("autohand.automode.resume", json!({}))
            .await
    }

    /// Cancels the active auto-mode session.
    pub async fn cancel_automode(
        &self,
        params: AutomodeCancelParams,
    ) -> Result<AutomodeCancelResult> {
        self.request_typed("autohand.automode.cancel", params).await
    }

    /// Returns auto-mode iteration records, optionally limited by the caller.
    pub async fn get_automode_log(
        &self,
        params: AutomodeGetLogParams,
    ) -> Result<AutomodeGetLogResult> {
        self.request_typed("autohand.automode.getLog", params).await
    }

    pub async fn get_skills_registry(
        &self,
        params: GetSkillsRegistryParams,
    ) -> Result<GetSkillsRegistryResult> {
        self.request_typed("autohand.getSkillsRegistry", params)
            .await
    }

    pub async fn install_skill(&self, params: InstallSkillParams) -> Result<InstallSkillResult> {
        if params.skill_name.trim().is_empty() {
            return Err(Error::InvalidInput("skill_name is required".into()));
        }
        self.request_typed("autohand.installSkill", params).await
    }

    pub async fn list_mcp_servers(&self) -> Result<McpListServersResult> {
        self.request_typed("autohand.mcp.listServers", json!({}))
            .await
    }

    pub async fn list_mcp_tools(&self, params: McpListToolsParams) -> Result<McpListToolsResult> {
        self.request_typed("autohand.mcp.listTools", params).await
    }

    pub async fn get_mcp_server_configs(&self) -> Result<McpGetServerConfigsResult> {
        self.request_typed("autohand.mcp.getServerConfigs", json!({}))
            .await
    }

    /// Initializes or resumes a persisted autoresearch loop.
    pub async fn start_autoresearch(
        &self,
        params: AutoresearchStartParams,
    ) -> Result<AutoresearchStartResult> {
        self.request_typed("autohand.autoresearch.start", params)
            .await
    }

    /// Returns current persisted autoresearch state.
    pub async fn get_autoresearch_status(&self) -> Result<AutoresearchStatusResult> {
        self.request_typed("autohand.autoresearch.status", json!({}))
            .await
    }

    /// Pauses autoresearch without deleting persisted state.
    pub async fn stop_autoresearch(&self) -> Result<AutoresearchStopResult> {
        self.request_typed("autohand.autoresearch.stop", json!({}))
            .await
    }

    /// Lists persisted autoresearch attempts.
    pub async fn get_autoresearch_history(&self) -> Result<AutoresearchHistoryResult> {
        self.request_typed("autohand.autoresearch.history", json!({}))
            .await
    }

    /// Re-evaluates a candidate in an isolated worktree.
    pub async fn replay_autoresearch(
        &self,
        params: AutoresearchReplayParams,
    ) -> Result<AutoresearchReplayResult> {
        self.request_typed("autohand.autoresearch.replay", params)
            .await
    }

    /// Reapplies current decision policy to persisted measurements.
    pub async fn rescore_autoresearch(
        &self,
        params: AutoresearchRescoreParams,
    ) -> Result<AutoresearchRescoreResult> {
        self.request_typed("autohand.autoresearch.rescore", params)
            .await
    }

    /// Compares persisted evidence for two attempts.
    pub async fn compare_autoresearch(
        &self,
        params: AutoresearchCompareParams,
    ) -> Result<AutoresearchCompareResult> {
        self.request_typed("autohand.autoresearch.compare", params)
            .await
    }

    /// Returns the current constraint-passing Pareto frontier.
    pub async fn get_autoresearch_pareto(&self) -> Result<AutoresearchParetoResult> {
        self.request_typed("autohand.autoresearch.pareto", json!({}))
            .await
    }

    /// Pins or unpins a candidate's replay artifacts.
    pub async fn pin_autoresearch(
        &self,
        params: AutoresearchPinParams,
    ) -> Result<AutoresearchPinResult> {
        self.request_typed("autohand.autoresearch.pin", params)
            .await
    }

    /// Previews or applies artifact retention.
    pub async fn prune_autoresearch(
        &self,
        params: AutoresearchPruneParams,
    ) -> Result<AutoresearchPruneResult> {
        self.request_typed("autohand.autoresearch.prune", params)
            .await
    }

    pub async fn permission_response(
        &self,
        request_id: impl Into<String>,
        decision: impl Into<String>,
    ) -> Result<Value> {
        self.request(
            "autohand.permissionResponse",
            json!({ "requestId": request_id.into(), "decision": decision.into() }),
        )
        .await
    }

    /// Confirms that a permission request reached the SDK client. The request
    /// must still be answered with [`Self::permission_response`].
    pub async fn acknowledge_permission(
        &self,
        request_id: impl Into<String>,
    ) -> Result<crate::PermissionAcknowledgedResult> {
        let request_id = request_id.into();
        if request_id.trim().is_empty() {
            return Err(Error::InvalidInput(
                "permission request_id is required".into(),
            ));
        }
        self.request_typed(
            "autohand.permissionAcknowledged",
            json!({ "requestId": request_id }),
        )
        .await
    }

    /// Allows or denies a pending request for access to an additional
    /// directory.
    pub async fn respond_to_directory_access(
        &self,
        request_id: impl Into<String>,
        granted: bool,
    ) -> Result<crate::DirectoryAccessResponseResult> {
        let request_id = request_id.into();
        if request_id.trim().is_empty() {
            return Err(Error::InvalidInput(
                "directory access request_id is required".into(),
            ));
        }
        self.request_typed(
            "autohand.directoryAccessResponse",
            json!({ "requestId": request_id, "granted": granted }),
        )
        .await
    }

    /// Confirms that a directory access request reached the SDK client.
    pub async fn acknowledge_directory_access(
        &self,
        request_id: impl Into<String>,
    ) -> Result<crate::DirectoryAccessAcknowledgedResult> {
        let request_id = request_id.into();
        if request_id.trim().is_empty() {
            return Err(Error::InvalidInput(
                "directory access request_id is required".into(),
            ));
        }
        self.request_typed(
            "autohand.directoryAccessAcknowledged",
            json!({ "requestId": request_id }),
        )
        .await
    }

    /// Applies or rejects a pending multi-file preview batch.
    pub async fn decide_changes(
        &self,
        params: crate::ChangesDecisionParams,
    ) -> Result<crate::ChangesDecisionResult> {
        params
            .validate()
            .map_err(|message| Error::InvalidInput(message.into()))?;
        self.request_typed("autohand.changesDecision", params).await
    }

    /// Returns a page of stored CLI sessions.
    pub async fn get_history(
        &self,
        params: crate::GetHistoryParams,
    ) -> Result<crate::GetHistoryResult> {
        self.request_typed("autohand.getHistory", params).await
    }

    /// Returns complete stored session details or an explicit lookup failure.
    pub async fn get_session(
        &self,
        session_id: impl Into<String>,
    ) -> Result<crate::SessionLookupResult> {
        let session_id = session_id.into();
        if session_id.trim().is_empty() {
            return Err(Error::InvalidInput("session_id is required".into()));
        }
        self.request_typed("autohand.getSession", json!({ "sessionId": session_id }))
            .await
    }

    /// Restores a stored session into the active RPC process.
    pub async fn attach_session(
        &self,
        session_id: impl Into<String>,
    ) -> Result<crate::SessionAttachResult> {
        let session_id = session_id.into();
        if session_id.trim().is_empty() {
            return Err(Error::InvalidInput("session_id is required".into()));
        }
        self.request_typed(
            "autohand.session.attach",
            json!({ "sessionId": session_id }),
        )
        .await
    }

    async fn set_yolo_with_method(
        &self,
        method: &str,
        params: crate::YoloSetParams,
    ) -> Result<crate::YoloSetResult> {
        self.request_typed(method, params).await
    }

    /// Sets timed unrestricted mode through the canonical CLI method.
    pub async fn set_yolo(&self, params: crate::YoloSetParams) -> Result<crate::YoloSetResult> {
        self.set_yolo_with_method("autohand.yoloSet", params).await
    }

    /// Sets timed unrestricted mode through the dotted compatibility alias.
    pub async fn set_yolo_alias(
        &self,
        params: crate::YoloSetParams,
    ) -> Result<crate::YoloSetResult> {
        self.set_yolo_with_method("autohand.yolo.set", params).await
    }

    /// Replaces the extension-provided MCP tool descriptors.
    pub async fn set_vscode_mcp_tools(
        &self,
        params: crate::McpSetVsCodeToolsParams,
    ) -> Result<crate::McpSetVsCodeToolsResult> {
        params
            .validate()
            .map_err(|message| Error::InvalidInput(message.into()))?;
        self.request_typed("autohand.mcp.setVscodeTools", params)
            .await
    }

    /// Completes a VS Code MCP invocation requested by the CLI.
    pub async fn respond_to_mcp_invocation(
        &self,
        params: crate::McpInvocationResponseParams,
    ) -> Result<crate::McpInvocationResponseResult> {
        params
            .validate()
            .map_err(|message| Error::InvalidInput(message.into()))?;
        self.request_typed("autohand.mcp.invokeResponse", params)
            .await
    }

    /// Audits project skills and returns scored registry recommendations.
    pub async fn recommend_project_learning(
        &self,
        params: crate::LearnRecommendParams,
    ) -> Result<crate::LearnRecommendResult> {
        self.request_typed("autohand.learn.recommend", params).await
    }

    /// Updates installed project skills from the registry.
    pub async fn update_project_learning(&self) -> Result<crate::LearnUpdateResult> {
        self.request_typed("autohand.learn.update", json!({})).await
    }

    /// Synthesizes and installs a skill from observed project workflows.
    pub async fn generate_project_skill(
        &self,
        params: crate::LearnGenerateParams,
    ) -> Result<crate::LearnGenerateResult> {
        self.request_typed("autohand.learn.generate", params).await
    }

    /// Returns all registered tools and definition diagnostics.
    pub async fn get_tools_registry(&self) -> Result<crate::GetToolsRegistryResult> {
        self.request_typed("autohand.getToolsRegistry", json!({}))
            .await
    }

    /// Enables or disables automatic context compaction.
    pub async fn set_context_compact(&self, enabled: bool) -> Result<crate::ContextCompactResult> {
        self.request_typed("autohand.setContextCompact", json!({ "enabled": enabled }))
            .await
    }

    fn inner(&self) -> Result<Arc<TransportInner>> {
        self.lifecycle
            .inner
            .lock()
            .map_err(|_| Error::LifecyclePoisoned)?
            .clone()
            .ok_or(Error::TransportNotStarted)
    }

    async fn request_typed<P, R>(&self, method: &str, params: P) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let params = serde_json::to_value(params)?;
        let result = self.request(method, params).await?;
        Ok(serde_json::from_value(result)?)
    }
}

fn is_final_stream_event(event: &SdkEvent) -> bool {
    matches!(event.event_type.as_str(), "agent_end" | "turn_end")
}

const PROMPT_QUEUED: u8 = 0;
const PROMPT_ACTIVE: u8 = 1;
const PROMPT_FINISHED: u8 = 2;
const PROMPT_CANCELLED: u8 = 3;

pub(crate) struct PromptStream {
    pub events: mpsc::Receiver<Result<SdkEvent>>,
    pub control: Arc<PromptControl>,
    pub response: oneshot::Receiver<Value>,
}

pub(crate) struct PromptControl {
    state: AtomicU8,
    cancelled: Notify,
    inner: Weak<TransportInner>,
    lifecycle: Weak<Lifecycle>,
}

impl PromptControl {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::SeqCst) == PROMPT_CANCELLED
    }

    fn finish(&self) {
        let _ = self.state.compare_exchange(
            PROMPT_ACTIVE,
            PROMPT_FINISHED,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    // The state transition and generation identity prevent a queued/old run
    // from terminating another turn or a replacement subprocess.
    fn cancel_inner(&self) -> Option<Arc<TransportInner>> {
        let previous = self
            .state
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |state| {
                (state == PROMPT_QUEUED || state == PROMPT_ACTIVE).then_some(PROMPT_CANCELLED)
            })
            .ok()?;
        self.cancelled.notify_one();
        if previous != PROMPT_ACTIVE {
            return None;
        }
        let inner = self.inner.upgrade()?;
        if let Some(lifecycle) = self.lifecycle.upgrade() {
            if let Ok(mut active) = lifecycle.inner.lock() {
                if active
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &inner))
                {
                    active.take();
                }
            }
        }
        inner.process_tree.terminate_now();
        inner.fail_pending();
        Some(inner)
    }

    pub(crate) fn cancel_now(&self) {
        self.cancel_inner();
    }

    pub(crate) async fn abort(&self) -> Result<()> {
        if let Some(inner) = self.cancel_inner() {
            inner.terminate().await?;
        }
        Ok(())
    }
}

struct TransportInner {
    config: Config,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: StdMutex<HashMap<u64, oneshot::Sender<Result<Value>>>>,
    startup_failure: StdMutex<Option<StartupFailure>>,
    next_id: AtomicU64,
    events: broadcast::Sender<SdkEvent>,
    process_tree: ProcessTree,
    prompt_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone)]
enum StartupFailure {
    AuthenticationRequired {
        code: i64,
        message: String,
        retryable: bool,
        provider_id: Option<String>,
    },
    InitializationFailed {
        code: i64,
        message: String,
        stage: String,
        retryable: bool,
    },
    Protocol {
        message: String,
    },
}

impl StartupFailure {
    fn from_rpc(error: &Value) -> Option<Self> {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Autohand CLI startup failed")
            .to_owned();
        let data = error.get("data");
        let kind = data
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str);
        let retryable = data
            .and_then(|value| value.get("retryable"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if code == -32011 || kind == Some("authentication_required") {
            return Some(Self::AuthenticationRequired {
                code,
                message,
                retryable,
                provider_id: data
                    .and_then(|value| value.get("providerId"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            });
        }
        if code != -32010 && kind != Some("initialization_failed") {
            return None;
        }
        Some(Self::InitializationFailed {
            code,
            message,
            stage: data
                .and_then(|value| value.get("stage"))
                .and_then(Value::as_str)
                .unwrap_or("startup")
                .to_owned(),
            retryable,
        })
    }

    fn to_error(&self) -> Error {
        match self {
            Self::AuthenticationRequired {
                code,
                message,
                retryable,
                provider_id,
            } => Error::AuthenticationRequired {
                code: *code,
                message: message.clone(),
                retryable: *retryable,
                provider_id: provider_id.clone(),
            },
            Self::InitializationFailed {
                code,
                message,
                stage,
                retryable,
            } => Error::InitializationFailed {
                code: *code,
                message: message.clone(),
                stage: stage.clone(),
                retryable: *retryable,
            },
            Self::Protocol { message } => Error::Protocol(message.clone()),
        }
    }
}

struct PendingRequestGuard<'a> {
    pending: &'a StdMutex<HashMap<u64, oneshot::Sender<Result<Value>>>>,
    id: u64,
}

impl Drop for PendingRequestGuard<'_> {
    fn drop(&mut self) {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.id);
    }
}

impl TransportInner {
    async fn start(config: Config) -> Result<Arc<Self>> {
        let cli = config
            .cli_path
            .clone()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "autohand".to_string());
        let mut command = command_for_network_policy(&config, &cli)?;
        match config.runtime_profile {
            RuntimeProfile::Interactive => {
                if let Some(cwd) = &config.cwd {
                    command.current_dir(cwd);
                }
            }
            RuntimeProfile::AnswerOnly(_) | RuntimeProfile::SetupOnly(_) => {
                command.current_dir(std::env::temp_dir());
            }
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if config.clear_environment {
            command.env_clear();
            for (key, value) in config.inherited_environment() {
                command.env(key, value);
            }
        }
        for (key, value) in config.cli_env() {
            command.env(key, value);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }

        let mut child = command.spawn()?;
        let process_tree = ProcessTree::attach(&child)?;
        let stdin = child.stdin.take().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "failed to open child stdin",
            ))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "failed to open child stdout",
            ))
        })?;
        let stderr = child.stderr.take();
        let (events, _) = broadcast::channel(512);
        let inner = Arc::new(Self {
            config,
            child: Mutex::new(child),
            stdin: Mutex::new(Some(stdin)),
            pending: StdMutex::new(HashMap::new()),
            startup_failure: StdMutex::new(None),
            next_id: AtomicU64::new(1),
            events,
            process_tree,
            prompt_lock: Arc::new(Mutex::new(())),
        });
        Self::spawn_stdout_reader(
            Arc::downgrade(&inner),
            stdout,
            inner.config.max_stdout_bytes,
        );
        if let Some(stderr) = stderr {
            Self::spawn_stderr_reader(
                Arc::downgrade(&inner),
                stderr,
                inner.config.max_stderr_bytes,
                inner.config.debug,
            );
        }
        Ok(inner)
    }

    async fn pump_prompt(
        self: &Arc<Self>,
        tx: &mpsc::Sender<Result<SdkEvent>>,
        control: &PromptControl,
        events: &mut broadcast::Receiver<SdkEvent>,
        params: Value,
        conditions: Vec<StopCondition>,
        response: oneshot::Sender<Value>,
    ) -> (Result<()>, bool) {
        let request_started = AtomicBool::new(false);
        let request = async {
            request_started.store(true, Ordering::SeqCst);
            self.request("autohand.prompt", params).await
        };
        tokio::pin!(request);
        let mut acknowledged = false;
        let mut terminal = false;
        let mut rejected = false;
        let mut response = Some(response);
        let mut steps = Arc::new(Vec::new());
        let mut decisions = JoinSet::new();
        let mut predicate_error = None;
        let grace_duration = self.config.timeout.min(Duration::from_secs(1));
        let failure_grace = time::sleep(grace_duration);
        tokio::pin!(failure_grace);
        let mut failed = false;
        let result = async {
            loop {
                if terminal && acknowledged && decisions.is_empty() {
                    return predicate_error.take().map_or(Ok(()), Err);
                }
                tokio::select! {
                    biased;
                    _ = tx.closed() => return Ok(()),
                    _ = control.cancelled.notified() => return Ok(()),
                    result = decisions.join_next(), if !decisions.is_empty() => {
                        if let Some(result) = result {
                            if let Some(error) = result?? {
                                predicate_error = Some(error);
                                failed = true;
                                failure_grace.as_mut().reset(time::Instant::now() + Duration::from_secs(2));
                            }
                        }
                    }
                    event = events.recv() => {
                        let event = match event {
                            Ok(event) => event,
                            Err(broadcast::error::RecvError::Lagged(count)) => return Err(Error::EventStreamLagged { count }),
                            Err(broadcast::error::RecvError::Closed) => return Err(Error::ChannelClosed),
                        };
                        if event.event_type == "transport_closed" { return Err(Error::MissingTerminalEvent); }
                        if let Some(step) = event.step_end() {
                            let step = step?;
                            if step.step_id.is_empty() { return Err(Error::Protocol("stepId must be non-empty".into())); }
                            // A valid next step follows acknowledgement of the previous decision.
                            let previous = tokio::select! {
                                biased;
                                _ = tx.closed() => return Ok(()),
                                _ = control.cancelled.notified() => return Ok(()),
                                previous = decisions.join_next() => previous,
                            };
                            if let Some(result) = previous {
                                if let Some(error) = result?? { return Err(error); }
                            }
                            Arc::make_mut(&mut steps).push(step.step.clone());
                            if !conditions.is_empty() {
                                let inner = self.clone();
                                let conditions = conditions.clone();
                                let context = StopConditionContext { steps: steps.clone() };
                                decisions.spawn(async move { inner.decide_step(step, context, conditions).await });
                            }
                        }
                        terminal |= is_final_stream_event(&event);
                        if terminal && matches!(event.raw.get("reason").and_then(Value::as_str), Some("aborted" | "error" | "failed" | "cancelled")) {
                            decisions.abort_all();
                            while decisions.join_next().await.is_some() {}
                        }
                        if event.event_type == "error" {
                            failed = true;
                            failure_grace.as_mut().reset(time::Instant::now() + grace_duration);
                        }
                        tokio::select! {
                            biased;
                            _ = control.cancelled.notified() => return Ok(()),
                            sent = tx.send(Ok(event)) => if sent.is_err() { return Ok(()); },
                        }
                    }
                    _ = &mut failure_grace, if failed => return predicate_error.take().map_or(Ok(()), Err),
                    result = &mut request, if !acknowledged => {
                        let value = match result {
                            Ok(value) => value,
                            Err(error) => {
                                rejected = matches!(error, Error::Rpc { .. } | Error::AuthenticationRequired { .. });
                                return Err(error);
                            }
                        };
                        if let Some(response) = response.take() { let _ = response.send(value); }
                        acknowledged = true;
                    }
                }
            }
        }.await;
        decisions.abort_all();
        while decisions.join_next().await.is_some() {}
        (
            result,
            terminal || rejected || !request_started.load(Ordering::SeqCst),
        )
    }

    async fn decide_step(
        &self,
        step: StepEndEvent,
        context: StopConditionContext,
        conditions: Vec<StopCondition>,
    ) -> Result<Option<Error>> {
        let mut stop = false;
        for condition in conditions {
            match condition.evaluate(context.clone()).await {
                Ok(true) => {
                    stop = true;
                    break;
                }
                Ok(false) => {}
                Err(error) => {
                    return match self.step_decision(&step.step_id, true).await {
                        Ok(()) => Ok(Some(error)),
                        Err(_) => Err(error),
                    };
                }
            }
        }
        self.step_decision(&step.step_id, stop).await?;
        Ok(None)
    }

    async fn step_decision(&self, step_id: &str, stop: bool) -> Result<()> {
        let result = self
            .request(
                "autohand.stepDecision",
                json!({"stepId": step_id, "stop": stop}),
            )
            .await?;
        if result.get("success") != Some(&Value::Bool(true)) {
            return Err(Error::Protocol(
                "autohand.stepDecision was rejected or returned an invalid result".into(),
            ));
        }
        Ok(())
    }

    async fn settle_prompt(&self, events: &mut broadcast::Receiver<SdkEvent>) {
        let settled = time::timeout(Duration::from_secs(2), async {
            self.request("autohand.abort", json!({})).await?;
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    // Notifications can overflow again while abort is acknowledged.
                    // Keep draining to a terminal within the original deadline.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return Err(Error::ChannelClosed),
                };
                if is_final_stream_event(&event) {
                    return Ok::<_, Error>(());
                }
                if event.event_type == "transport_closed" {
                    return Err(Error::ChannelClosed);
                }
            }
        })
        .await;
        if !matches!(settled, Ok(Ok(()))) {
            let _ = self.terminate().await;
        }
    }

    async fn stop(&self) -> Result<()> {
        {
            let mut stdin = self.stdin.lock().await;
            if let Some(mut pipe) = stdin.take() {
                let _ = pipe.shutdown().await;
            }
        }

        let mut child = self.child.lock().await;
        if child.id().is_some() {
            match time::timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(_status)) => {
                    self.process_tree.terminate_now();
                }
                Ok(Err(error)) => return Err(Error::Io(error)),
                Err(_) => {
                    self.process_tree.terminate_now();
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
            }
        }
        drop(child);
        self.fail_pending();
        Ok(())
    }

    async fn terminate(&self) -> Result<()> {
        self.process_tree.terminate_now();
        {
            let mut stdin = self.stdin.lock().await;
            stdin.take();
        }
        let mut child = self.child.lock().await;
        if child.id().is_some() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        self.fail_pending();
        Ok(())
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value> {
        if let Some(failure) = self
            .startup_failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
        {
            return Err(failure.to_error());
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&message)?;
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, tx);
        let _pending = PendingRequestGuard {
            pending: &self.pending,
            id,
        };

        let write_result = async {
            let mut stdin = self.stdin.lock().await;
            let stdin = stdin.as_mut().ok_or(Error::ChannelClosed)?;
            stdin.write_all(line.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
            Ok::<(), Error>(())
        }
        .await;
        write_result?;

        match time::timeout(self.config.timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Error::ChannelClosed),
            Err(_) => {
                self.process_tree.terminate_now();
                self.fail_pending();
                Err(Error::RequestTimeout(method.to_string()))
            }
        }
    }

    fn spawn_stdout_reader(
        inner: Weak<Self>,
        stdout: tokio::process::ChildStdout,
        max_stdout_bytes: usize,
    ) {
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut stdout_bytes = 0;
            loop {
                let line =
                    read_bounded_line(&mut reader, &mut stdout_bytes, max_stdout_bytes, "stdout")
                        .await;
                let Some(inner) = inner.upgrade() else {
                    return;
                };
                match line {
                    Ok(Some(line)) => {
                        if let Err(error) = inner.handle_line(&line) {
                            let message = match error {
                                Error::Protocol(message) => message,
                                Error::Json(error) => {
                                    format!("stdout JSON frame was invalid: {error}")
                                }
                                error => error.to_string(),
                            };
                            let _ = inner.events.send(SdkEvent::new(
                                "error",
                                json!({ "type": "error", "message": message }),
                            ));
                            inner.process_tree.terminate_now();
                            inner.fail_pending_with(|| Error::Protocol(message.clone()));
                            return;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let message = match &error {
                            Error::Protocol(message) => message.clone(),
                            Error::Io(error) => format!("stdout read failed: {error}"),
                            error => error.to_string(),
                        };
                        let _ = inner.events.send(SdkEvent::new(
                            "error",
                            json!({ "type": "error", "message": message }),
                        ));
                        inner.process_tree.terminate_now();
                        if matches!(error, Error::OutputLimitExceeded { .. }) {
                            inner.fail_pending_with(|| Error::OutputLimitExceeded {
                                stream: "stdout",
                                limit: max_stdout_bytes,
                            });
                        } else {
                            inner.fail_pending_with(|| Error::Protocol(message.clone()));
                        }
                        return;
                    }
                }
            }
            if let Some(inner) = inner.upgrade() {
                inner.process_tree.terminate_now();
                let _ = inner.events.send(SdkEvent::new(
                    "transport_closed",
                    json!({ "type": "transport_closed" }),
                ));
                inner.fail_pending();
            }
        });
    }

    fn spawn_stderr_reader(
        inner: Weak<Self>,
        stderr: tokio::process::ChildStderr,
        max_stderr_bytes: usize,
        debug: bool,
    ) {
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut stderr_bytes = 0;
            loop {
                let line =
                    read_bounded_line(&mut reader, &mut stderr_bytes, max_stderr_bytes, "stderr")
                        .await;
                let Some(inner) = inner.upgrade() else {
                    return;
                };
                match line {
                    Ok(Some(line)) => {
                        if debug {
                            eprintln!("[autohand] {line}");
                        }
                    }
                    Ok(None) => return,
                    Err(error) => {
                        inner.process_tree.terminate_now();
                        if matches!(error, Error::OutputLimitExceeded { .. }) {
                            inner.fail_pending_with(|| Error::OutputLimitExceeded {
                                stream: "stderr",
                                limit: max_stderr_bytes,
                            });
                        } else {
                            let message = match error {
                                Error::Protocol(message) => message,
                                Error::Io(error) => format!("stderr read failed: {error}"),
                                error => error.to_string(),
                            };
                            inner.fail_pending_with(|| Error::Protocol(message.clone()));
                        }
                        return;
                    }
                }
            }
        });
    }

    fn fail_pending(&self) {
        let pending = std::mem::take(
            &mut *self
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        for (_, response) in pending {
            let _ = response.send(Err(Error::ChannelClosed));
        }
    }

    fn fail_pending_with(&self, mut error: impl FnMut() -> Error) {
        let pending = std::mem::take(
            &mut *self
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        for (_, response) in pending {
            let _ = response.send(Err(error()));
        }
    }

    fn handle_line(&self, line: &str) -> Result<()> {
        let value: Value = serde_json::from_str(line)?;
        let object = value
            .as_object()
            .ok_or_else(|| Error::Protocol("JSON-RPC frame must be one object".to_owned()))?;
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(Error::Protocol(
                "JSON-RPC frame omitted exact version 2.0".to_owned(),
            ));
        }
        if value.get("id") == Some(&Value::Null) {
            if object.contains_key("result") && object.contains_key("error") {
                return Err(Error::Protocol(
                    "JSON-RPC response cannot contain both result and error".to_owned(),
                ));
            }
            let failure = if matches!(self.config.runtime_profile, RuntimeProfile::SetupOnly(_)) {
                StartupFailure::Protocol {
                    message:
                        "setup-only CLI emitted an unbound error outside login contract version 1"
                            .to_owned(),
                }
            } else if let Some(error) = value.get("error") {
                let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
                let message = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown unbound RPC error");
                StartupFailure::from_rpc(error).unwrap_or_else(|| StartupFailure::Protocol {
                    message: format!("unbound RPC error {code}: {message}"),
                })
            } else {
                StartupFailure::Protocol {
                    message: "JSON-RPC response used a null id without an error".to_owned(),
                }
            };
            let terminate = matches!(failure, StartupFailure::Protocol { .. });
            *self
                .startup_failure
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(failure.clone());
            let pending = std::mem::take(
                &mut *self
                    .pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            );
            for (_, response) in pending {
                let _ = response.send(Err(failure.to_error()));
            }
            if terminate {
                self.process_tree.terminate_now();
            }
            return Ok(());
        }
        if let Some(id_value) = value.get("id") {
            let id = id_value.as_u64().ok_or_else(|| {
                Error::Protocol("JSON-RPC response id must be an unsigned integer".to_owned())
            })?;
            if object.contains_key("result") == object.contains_key("error") {
                return Err(Error::Protocol(
                    "JSON-RPC response requires exactly one of result or error".to_owned(),
                ));
            }
            if let Some(tx) = self
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&id)
            {
                if let Some(error) = value.get("error") {
                    let _ = tx.send(Err(rpc_error(error, self.config.runtime_profile)));
                } else {
                    let _ = tx.send(Ok(value.get("result").cloned().unwrap_or(Value::Null)));
                }
            }
            return Ok(());
        }

        if let Some(method) = value.get("method").and_then(Value::as_str) {
            if object.contains_key("result") || object.contains_key("error") {
                return Err(Error::Protocol(
                    "JSON-RPC notification cannot contain result or error".to_owned(),
                ));
            }
            let params = value.get("params").cloned().unwrap_or(Value::Null);
            let _ = self.events.send(event_from_notification(method, params));
            return Ok(());
        }
        Err(Error::Protocol(
            "JSON-RPC frame was neither a response nor a notification".to_owned(),
        ))
    }
}

fn rpc_error(error: &Value, profile: RuntimeProfile) -> Error {
    let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Unknown RPC error")
        .to_owned();
    let data = error.get("data");
    let kind = data
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str);
    if matches!(profile, RuntimeProfile::SetupOnly(_)) {
        if let Some(problem_code) = kind.and_then(LoginProblemCode::from_rpc_kind) {
            return match LoginProblem::from_rpc(
                problem_code,
                message,
                data.and_then(|value| value.get("retryable"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ) {
                Ok(problem) => Error::LoginFailed { problem },
                Err(_) => Error::Protocol(
                    "Autohand login failure contained unsafe public fields".to_owned(),
                ),
            };
        }
        return Error::LoginFailed {
            problem: LoginProblem {
                code: LoginProblemCode::ProtocolMismatch,
                message:
                    "The Autohand authorization response did not match setup contract version 1."
                        .to_owned(),
                retryable: false,
            },
        };
    }
    if code == -32011 || kind == Some("authentication_required") {
        return Error::AuthenticationRequired {
            code,
            message,
            retryable: data
                .and_then(|value| value.get("retryable"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            provider_id: data
                .and_then(|value| value.get("providerId"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        };
    }
    Error::Rpc {
        code,
        message,
        data: data.cloned(),
    }
}

impl Drop for TransportInner {
    fn drop(&mut self) {
        self.process_tree.terminate_now();
    }
}

async fn read_bounded_line<R>(
    reader: &mut R,
    total: &mut usize,
    limit: usize,
    stream: &'static str,
) -> Result<Option<String>>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if line.is_empty() {
                return Ok(None);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |index| index + 1);
        if total
            .checked_add(consumed)
            .map_or(true, |observed| observed > limit)
        {
            reader.consume(consumed);
            return Err(Error::OutputLimitExceeded { stream, limit });
        }
        *total += consumed;
        let content_end = newline.unwrap_or(consumed);
        line.extend_from_slice(&available[..content_end]);
        reader.consume(consumed);
        if newline.is_some() {
            break;
        }
    }
    String::from_utf8(line)
        .map(Some)
        .map_err(|_| Error::Protocol(format!("{stream} was not valid UTF-8")))
}

#[cfg(unix)]
struct ProcessTree {
    process_group: libc::pid_t,
    active: AtomicBool,
}

#[cfg(unix)]
impl ProcessTree {
    fn attach(child: &Child) -> Result<Self> {
        let process_group = child.id().ok_or_else(|| {
            Error::Protocol("spawned CLI did not expose a process identifier".to_owned())
        })? as libc::pid_t;
        Ok(Self {
            process_group,
            active: AtomicBool::new(true),
        })
    }

    fn terminate_now(&self) {
        if self.active.swap(false, Ordering::SeqCst) {
            unsafe {
                libc::kill(-self.process_group, libc::SIGKILL);
            }
        }
    }
}

#[cfg(windows)]
struct ProcessTree {
    job: windows_sys::Win32::Foundation::HANDLE,
    active: AtomicBool,
}

#[cfg(windows)]
unsafe impl Send for ProcessTree {}

#[cfg(windows)]
unsafe impl Sync for ProcessTree {}

#[cfg(windows)]
impl ProcessTree {
    fn attach(child: &Child) -> Result<Self> {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
        };

        unsafe {
            let child_handle = child.raw_handle().ok_or_else(|| {
                Error::Protocol("spawned CLI did not expose a process handle".to_owned())
            })? as isize;
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job == 0 {
                return Err(Error::Io(std::io::Error::last_os_error()));
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            );
            let assigned = configured != 0 && AssignProcessToJobObject(job, child_handle) != 0;
            if !assigned {
                let error = std::io::Error::last_os_error();
                CloseHandle(job);
                return Err(Error::Io(error));
            }
            Ok(Self {
                job,
                active: AtomicBool::new(true),
            })
        }
    }

    fn terminate_now(&self) {
        if self.active.swap(false, Ordering::SeqCst) {
            unsafe {
                windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1);
            }
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

fn rpc_method_allowed(profile: RuntimeProfile, method: &str) -> bool {
    match profile {
        RuntimeProfile::Interactive => true,
        RuntimeProfile::AnswerOnly(_) => {
            matches!(method, "autohand.runtimeInspect" | "autohand.answer")
        }
        RuntimeProfile::SetupOnly(_) => matches!(
            method,
            "autohand.login.begin" | "autohand.login.poll" | "autohand.login.cancel"
        ),
    }
}

fn profile_name(profile: RuntimeProfile) -> &'static str {
    match profile {
        RuntimeProfile::Interactive => "interactive",
        RuntimeProfile::AnswerOnly(_) => "answer_only",
        RuntimeProfile::SetupOnly(_) => "setup_only",
    }
}

fn command_for_network_policy(config: &Config, cli: &str) -> Result<Command> {
    match config.network_policy {
        crate::ChildNetworkPolicy::Inherit
        | crate::ChildNetworkPolicy::SetupOnly(
            crate::SetupTrafficClass::AutohandDeviceAuthorization,
        ) => {
            let mut command = Command::new(cli);
            command.args(config.cli_args());
            Ok(command)
        }
        crate::ChildNetworkPolicy::DenyAll => deny_all_command(config, cli),
    }
}

#[cfg(target_os = "macos")]
fn deny_all_command(config: &Config, cli: &str) -> Result<Command> {
    const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
    if !std::path::Path::new(SANDBOX_EXEC).is_file() {
        return Err(Error::NetworkPolicyUnavailable(
            "sandbox-exec is required for deny-all child egress on macOS".to_owned(),
        ));
    }
    let mut command = Command::new(SANDBOX_EXEC);
    command.args(["-p", "(version 1) (allow default) (deny network*)", cli]);
    command.args(config.cli_args());
    Ok(command)
}

#[cfg(target_os = "linux")]
fn deny_all_command(config: &Config, cli: &str) -> Result<Command> {
    let bwrap = ["/usr/bin/bwrap", "/bin/bwrap"]
        .iter()
        .find(|path| std::path::Path::new(path).is_file())
        .ok_or_else(|| {
            Error::NetworkPolicyUnavailable(
                "bubblewrap is required for deny-all child egress on Linux".to_owned(),
            )
        })?;
    let mut command = Command::new(bwrap);
    command.args([
        "--die-with-parent",
        "--unshare-net",
        "--bind",
        "/",
        "/",
        "--dev-bind",
        "/dev",
        "/dev",
        "--proc",
        "/proc",
        "--",
        cli,
    ]);
    command.args(config.cli_args());
    Ok(command)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn deny_all_command(_config: &Config, _cli: &str) -> Result<Command> {
    Err(Error::NetworkPolicyUnavailable(
        "deny-all child egress is not implemented on this platform".to_owned(),
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use std::{fs, hint::black_box, os::unix::fs::PermissionsExt, time::Instant};

    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn streams_events_from_fake_cli() {
        for (method, event_type) in [
            ("autohand.turnEnd", "turn_end"),
            ("autohand.agentEnd", "agent_end"),
        ] {
            assert_streams_events_from_fake_cli(method, event_type).await;
        }
    }

    async fn assert_streams_events_from_fake_cli(terminal_method: &str, terminal_type: &str) {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("fake-autohand");
        fs::write(
            &cli,
            r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.prompt*)
      prompt_id="$id"
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.permissionRequest","params":{"type":"permission_request","requestId":"perm-1","tool":"bash","description":"list files"}}'
      IFS= read -r permission_line || exit 1
      permission_id=$(printf '%s\n' "$permission_line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ok":true}}\n' "$permission_id"
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.messageUpdate","params":{"type":"message_update","delta":"hello"}}'
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.messageEnd","params":{"type":"message_end","content":"hello"}}'
      printf '{"jsonrpc":"2.0","method":"%s","params":{"type":"%s"}}\n' "$AUTOHAND_TEST_TERMINAL_METHOD" "$AUTOHAND_TEST_TERMINAL_TYPE"
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ok":true}}\n' "$prompt_id"
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ok":true}}\n' "$id"
      ;;
  esac
done
"#,
        )
        .unwrap();
        let mut perms = fs::metadata(&cli).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&cli, perms).unwrap();

        let mut config = Config::default().with_cli_path(&cli);
        config.env.insert(
            "AUTOHAND_TEST_TERMINAL_METHOD".into(),
            terminal_method.into(),
        );
        config
            .env
            .insert("AUTOHAND_TEST_TERMINAL_TYPE".into(), terminal_type.into());
        let mut sdk = AutohandSdk::new(config);
        sdk.start().await.unwrap();
        let mut events = sdk
            .stream_prompt("hello", PromptOptions::default())
            .await
            .unwrap();
        let mut text = String::new();
        let mut answered_permission = false;
        let mut event_types = Vec::new();
        while let Some(event) = events.recv().await {
            let event = event.unwrap();
            event_types.push(event.event_type.clone());
            if event.event_type == "permission_request" {
                sdk.permission_response(event.request_id().unwrap_or_default(), "allow_once")
                    .await
                    .unwrap();
                answered_permission = true;
            }
            if let Some(delta) = event.text_delta() {
                text.push_str(delta);
            }
        }
        sdk.stop().await.unwrap();
        assert_eq!(text, "hello");
        assert!(answered_permission);
        assert_eq!(
            event_types,
            [
                "permission_request",
                "message_update",
                "message_end",
                terminal_type
            ]
        );
    }

    fn write_fake_cli(path: &std::path::Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[tokio::test]
    async fn startup_failure_is_transactional_and_retryable() {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("fake-autohand");
        let marker = dir.path().join("failed-once");
        write_fake_cli(
            &cli,
            &format!(
                r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  if [ ! -e "{}" ]; then
    : > "{}"
    printf '{{"jsonrpc":"2.0","id":%s,"error":{{"code":-1,"message":"not ready"}}}}\n' "$id"
  else
    printf '{{"jsonrpc":"2.0","id":%s,"result":{{"ready":true}}}}\n' "$id"
  fi
done
"#,
                marker.display(),
                marker.display()
            ),
        );

        let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
        assert!(sdk.start().await.is_err());
        assert!(!sdk.is_started());
        sdk.start().await.unwrap();
        assert!(sdk.is_started());
        sdk.stop().await.unwrap();
    }

    #[tokio::test]
    async fn cloned_handles_share_lifecycle_state() {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("fake-autohand");
        write_fake_cli(
            &cli,
            r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  printf '{"jsonrpc":"2.0","id":%s,"result":{"ok":true}}\n' "$id"
done
"#,
        );

        let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
        sdk.start().await.unwrap();
        let mut clone = sdk.clone();
        assert!(clone.is_started());
        clone.stop().await.unwrap();
        assert!(!sdk.is_started());
        assert!(matches!(
            sdk.get_state().await,
            Err(Error::TransportNotStarted)
        ));
    }

    #[tokio::test]
    async fn dropping_last_sdk_handle_releases_transport() {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("fake-autohand");
        write_fake_cli(
            &cli,
            r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  printf '{"jsonrpc":"2.0","id":%s,"result":{"ok":true}}\n' "$id"
done
"#,
        );

        let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
        sdk.start().await.unwrap();
        let inner = sdk.inner().unwrap();
        let weak = Arc::downgrade(&inner);
        drop(inner);
        drop(sdk);
        tokio::task::yield_now().await;
        assert!(weak.upgrade().is_none());
    }

    #[tokio::test]
    async fn stdout_eof_fails_and_drains_pending_requests() {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("fake-autohand");
        write_fake_cli(
            &cli,
            r#"#!/bin/sh
IFS= read -r line || exit 1
id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
printf '{"jsonrpc":"2.0","id":%s,"result":{"ready":true}}\n' "$id"
IFS= read -r line || exit 1
exit 0
"#,
        );

        let mut config = Config::default().with_cli_path(&cli);
        config.timeout = Duration::from_secs(5);
        let mut sdk = AutohandSdk::new(config);
        sdk.start().await.unwrap();
        let inner = sdk.inner().unwrap();
        let result = time::timeout(
            Duration::from_secs(1),
            sdk.request("autohand.neverReplies", json!({})),
        )
        .await
        .expect("EOF should resolve the request without waiting for its timeout");
        assert!(matches!(result, Err(Error::ChannelClosed)));
        assert!(inner
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty());
        sdk.stop().await.unwrap();
    }

    #[tokio::test]
    async fn dropping_stream_receiver_cleans_up_pending_request() {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("fake-autohand");
        let marker = dir.path().join("prompt-received");
        write_fake_cli(
            &cli,
            &format!(
                r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.prompt*)
      : > "{}"
      IFS= read -r ignored || exit 0
      ;;
    *)
      printf '{{"jsonrpc":"2.0","id":%s,"result":{{"ready":true}}}}\n' "$id"
      ;;
  esac
done
"#,
                marker.display()
            ),
        );

        let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
        sdk.start().await.unwrap();
        let inner = sdk.inner().unwrap();
        let receiver = sdk
            .stream_prompt("hello", PromptOptions::default())
            .await
            .unwrap();

        time::timeout(Duration::from_secs(1), async {
            while !marker.exists() {
                time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the fixture should receive the prompt");
        assert_eq!(
            inner
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
            1
        );
        let request_id = *inner.pending.lock().unwrap().keys().next().unwrap();

        drop(receiver);
        time::timeout(Duration::from_secs(1), async {
            loop {
                if inner
                    .pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .get(&request_id)
                    .is_none()
                {
                    break;
                }
                time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("dropping the stream receiver should cancel and clean up its request");
        time::timeout(Duration::from_secs(3), async {
            loop {
                if inner.pending.lock().unwrap().is_empty() {
                    break;
                }
                time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("an unresponsive abort is bounded and retires the process");
        assert!(inner.child.lock().await.try_wait().unwrap().is_some());
        sdk.stop().await.unwrap();
    }

    #[tokio::test]
    async fn write_failure_removes_pending_request() {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("fake-autohand");
        write_fake_cli(
            &cli,
            r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  printf '{"jsonrpc":"2.0","id":%s,"result":{"ok":true}}\n' "$id"
done
"#,
        );

        let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
        sdk.start().await.unwrap();
        let inner = sdk.inner().unwrap();
        inner.stdin.lock().await.take();
        assert!(matches!(
            inner.request("autohand.closed", json!({})).await,
            Err(Error::ChannelClosed)
        ));
        assert!(inner
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty());
        sdk.stop().await.unwrap();
    }

    #[test]
    fn public_import_probe() {
        if std::env::var("AUTOHAND_RUST_PUBLIC_IMPORT_PROBE").as_deref() != Ok("1") {
            return;
        }
        let started = Instant::now();
        crate::initialize();
        println!("PUBLIC_IMPORT_NS={}", started.elapsed().as_nanos());
    }

    fn percentile_95(samples: &mut [Duration]) -> Duration {
        samples.sort_unstable();
        let index = (samples.len() * 95).div_ceil(100).saturating_sub(1);
        samples[index]
    }

    fn median(samples: &mut [Duration]) -> Duration {
        samples.sort_unstable();
        samples[samples.len() / 2]
    }

    fn maximum(samples: &[Duration]) -> Duration {
        samples.iter().copied().max().unwrap_or_default()
    }

    fn measure_public_import() -> Duration {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .env("AUTOHAND_RUST_PUBLIC_IMPORT_PROBE", "1")
            .args([
                "--exact",
                "transport::tests::public_import_probe",
                "--nocapture",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        let nanos = stdout
            .split_whitespace()
            .find_map(|word| word.strip_prefix("PUBLIC_IMPORT_NS="))
            .and_then(|value| value.parse::<u64>().ok())
            .expect("public import probe should report its internal timer");
        Duration::from_nanos(nanos)
    }

    async fn measure_sdk_start_return(cli: &std::path::Path) -> Duration {
        let mut sdk = AutohandSdk::new(Config::default().with_cli_path(cli));
        let started = Instant::now();
        sdk.start().await.unwrap();
        let elapsed = started.elapsed();
        sdk.stop().await.unwrap();
        elapsed
    }

    async fn measure_fixture_first_rpc(cli: &std::path::Path) -> Duration {
        let config = Config::default().with_cli_path(cli);
        let started = Instant::now();
        let inner = TransportInner::start(config).await.unwrap();
        inner.request("autohand.getState", json!({})).await.unwrap();
        let elapsed = started.elapsed();
        inner.stop().await.unwrap();
        elapsed
    }

    #[tokio::test]
    async fn startup_budgets() {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("fake-autohand");
        write_fake_cli(
            &cli,
            r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  printf '{"jsonrpc":"2.0","id":%s,"result":{"ready":true}}\n' "$id"
done
"#,
        );

        for _ in 0..5 {
            black_box(measure_public_import());
            black_box(measure_sdk_start_return(&cli).await);
            black_box(measure_fixture_first_rpc(&cli).await);
        }

        let mut public_import = Vec::with_capacity(50);
        let mut sdk_start = Vec::with_capacity(50);
        let mut fixture_first_rpc = Vec::with_capacity(50);
        for _ in 0..50 {
            public_import.push(measure_public_import());
            sdk_start.push(measure_sdk_start_return(&cli).await);
            fixture_first_rpc.push(measure_fixture_first_rpc(&cli).await);
        }

        let public_import_median = median(&mut public_import.clone());
        let sdk_start_median = median(&mut sdk_start.clone());
        let fixture_median = median(&mut fixture_first_rpc.clone());
        let public_import_p95 = percentile_95(&mut public_import);
        let sdk_start_p95 = percentile_95(&mut sdk_start);
        let fixture_p95 = percentile_95(&mut fixture_first_rpc);
        let budget = Duration::from_millis(50);
        let public_passed = public_import_p95 < budget;
        let sdk_passed = sdk_start_p95 < budget;
        let fixture_passed = fixture_p95 < budget;
        let to_ms = |value: Duration| value.as_secs_f64() * 1_000.0;
        let report = json!({
            "language": "rust",
            "budgetMs": 50,
            "metrics": {
                "publicImportMs": {
                    "samples": 50,
                    "medianMs": to_ms(public_import_median),
                    "p95Ms": to_ms(public_import_p95),
                    "maxMs": to_ms(maximum(&public_import)),
                    "passed": public_passed,
                },
                "sdkStartReturnMs": {
                    "samples": 50,
                    "medianMs": to_ms(sdk_start_median),
                    "p95Ms": to_ms(sdk_start_p95),
                    "maxMs": to_ms(maximum(&sdk_start)),
                    "passed": sdk_passed,
                },
                "fixtureSpawnToFirstRpcMs": {
                    "samples": 50,
                    "medianMs": to_ms(fixture_median),
                    "p95Ms": to_ms(fixture_p95),
                    "maxMs": to_ms(maximum(&fixture_first_rpc)),
                    "passed": fixture_passed,
                },
            },
            "passed": public_passed && sdk_passed && fixture_passed,
        });
        println!("{report}");

        assert!(public_passed, "publicImportMs p95 exceeded 50ms");
        assert!(sdk_passed, "sdkStartReturnMs p95 exceeded 50ms");
        assert!(fixture_passed, "fixtureSpawnToFirstRpcMs p95 exceeded 50ms");
    }
}
