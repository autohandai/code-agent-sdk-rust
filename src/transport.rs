use std::{
    collections::HashMap,
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex as StdMutex, Weak,
    },
    time::Duration,
};

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{broadcast, oneshot, Mutex},
    time,
};

use crate::{
    config::PromptOptions, event::event_from_notification, AutoresearchCompareParams,
    AutoresearchCompareResult, AutoresearchHistoryResult, AutoresearchParetoResult,
    AutoresearchPinParams, AutoresearchPinResult, AutoresearchPruneParams, AutoresearchPruneResult,
    AutoresearchReplayParams, AutoresearchReplayResult, AutoresearchRescoreParams,
    AutoresearchRescoreResult, AutoresearchStartParams, AutoresearchStartResult,
    AutoresearchStatusResult, AutoresearchStopResult, Config, Error, GetSkillsRegistryParams,
    GetSkillsRegistryResult, GoalCreateParams, GoalMutationResult, GoalSnapshot,
    GoalTemplateMetadata, GoalUpdateParams, InstallSkillParams, InstallSkillResult,
    McpGetServerConfigsResult, McpListServersResult, McpListToolsParams, McpListToolsResult,
    ResetResult, Result, SdkEvent,
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

        let inner = TransportInner::start(self.config.clone()).await?;
        let initialize = async {
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
        .await;
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

    pub fn is_started(&self) -> bool {
        self.lifecycle
            .inner
            .lock()
            .map(|inner| inner.is_some())
            .unwrap_or(false)
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        self.inner()?.request(method, params).await
    }

    pub async fn prompt(
        &self,
        message: impl Into<String>,
        options: PromptOptions,
    ) -> Result<Value> {
        self.request("autohand.prompt", options.to_params(message))
            .await
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
        let inner = self.inner()?.clone();
        let mut events = inner.events.subscribe();
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let params = options.to_params(message);

        tokio::spawn(async move {
            let request = inner.request("autohand.prompt", params);
            tokio::pin!(request);
            let mut request_done = false;
            let mut stream_done = false;
            loop {
                tokio::select! {
                    biased;
                    _ = tx.closed() => {
                        break;
                    }
                    event = events.recv() => {
                        match event {
                            Ok(event) => {
                                let terminal = is_terminal_stream_event(&event);
                                if tx.send(Ok(event)).await.is_err() {
                                    break;
                                }
                                if terminal {
                                    stream_done = true;
                                    if request_done {
                                        break;
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => {
                                if !stream_done {
                                    let _ = tx.send(Err(Error::ChannelClosed)).await;
                                }
                                break;
                            }
                        }
                    }
                    result = &mut request, if !request_done => {
                        if let Err(error) = result {
                            let _ = tx.send(Err(error)).await;
                            break;
                        }
                        request_done = true;
                        while let Ok(event) = events.try_recv() {
                            let terminal = is_terminal_stream_event(&event);
                            if tx.send(Ok(event)).await.is_err() {
                                break;
                            }
                            if terminal {
                                stream_done = true;
                            }
                        }
                        if stream_done {
                            break;
                        }
                    }
                }
            }
        });

        Ok(rx)
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

fn is_terminal_stream_event(event: &SdkEvent) -> bool {
    matches!(event.event_type.as_str(), "agent_end" | "error")
}

struct TransportInner {
    config: Config,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: StdMutex<HashMap<u64, oneshot::Sender<Result<Value>>>>,
    next_id: AtomicU64,
    events: broadcast::Sender<SdkEvent>,
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
        let mut command = Command::new(cli);
        command.args(config.cli_args());
        if let Some(cwd) = &config.cwd {
            command.current_dir(cwd);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (key, value) in config.cli_env() {
            command.env(key, value);
        }

        let mut child = command.spawn()?;
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
            next_id: AtomicU64::new(1),
            events,
        });
        Self::spawn_stdout_reader(Arc::downgrade(&inner), stdout);
        if let Some(stderr) = stderr {
            Self::spawn_stderr_reader(inner.config.debug, stderr);
        }
        Ok(inner)
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
                Ok(Ok(_status)) => {}
                Ok(Err(error)) => return Err(Error::Io(error)),
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
            }
        }
        drop(child);
        self.fail_pending();
        Ok(())
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value> {
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
            Err(_) => Err(Error::RequestTimeout(method.to_string())),
        }
    }

    fn spawn_stdout_reader(inner: Weak<Self>, stdout: tokio::process::ChildStdout) {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Some(inner) = inner.upgrade() else {
                    return;
                };
                if let Err(error) = inner.handle_line(&line) {
                    let _ = inner.events.send(SdkEvent::new(
                        "error",
                        json!({ "type": "error", "message": error.to_string() }),
                    ));
                }
            }
            if let Some(inner) = inner.upgrade() {
                inner.fail_pending();
            }
        });
    }

    fn spawn_stderr_reader(debug: bool, stderr: tokio::process::ChildStderr) {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if debug {
                    eprintln!("[autohand] {line}");
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

    fn handle_line(&self, line: &str) -> Result<()> {
        let value: Value = serde_json::from_str(line)?;
        if let Some(id) = value.get("id").and_then(Value::as_u64) {
            if let Some(tx) = self
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&id)
            {
                if let Some(error) = value.get("error") {
                    let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
                    let message = error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Unknown RPC error")
                        .to_string();
                    let data = error.get("data").cloned();
                    let _ = tx.send(Err(Error::Rpc {
                        code,
                        message,
                        data,
                    }));
                } else {
                    let _ = tx.send(Ok(value.get("result").cloned().unwrap_or(Value::Null)));
                }
            }
            return Ok(());
        }

        if let Some(method) = value.get("method").and_then(Value::as_str) {
            let params = value.get("params").cloned().unwrap_or(Value::Null);
            let _ = self.events.send(event_from_notification(method, params));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, hint::black_box, os::unix::fs::PermissionsExt, time::Instant};

    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn streams_events_from_fake_cli() {
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
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.turnEnd","params":{"type":"turn_end"}}'
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ok":true}}\n' "$prompt_id"
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.agentEnd","params":{"type":"agent_end"}}'
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

        let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
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
        assert!(event_types.iter().any(|kind| kind == "message_end"));
        assert!(event_types.iter().any(|kind| kind == "turn_end"));
        assert_eq!(event_types.last().map(String::as_str), Some("agent_end"));
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

        drop(receiver);
        time::timeout(Duration::from_secs(1), async {
            loop {
                if inner
                    .pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .is_empty()
                {
                    break;
                }
                time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("dropping the stream receiver should cancel and clean up its request");
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
