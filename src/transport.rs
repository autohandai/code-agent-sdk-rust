use std::{
    collections::HashMap,
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
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
    AutoresearchStatusResult, AutoresearchStopResult, Config, Error, GoalCreateParams,
    GoalMutationResult, GoalSnapshot, GoalTemplateMetadata, GoalUpdateParams, Result, SdkEvent,
};

#[derive(Clone)]
pub struct AutohandSdk {
    config: Config,
    inner: Option<Arc<TransportInner>>,
}

impl AutohandSdk {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            inner: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.inner.is_some() {
            return Ok(());
        }
        self.inner = Some(TransportInner::start(self.config.clone()).await?);
        if let Some(features) = &self.config.features {
            self.request(
                "autohand.applyFlagSettings",
                json!({"settings":{"features":features}}),
            )
            .await?;
        }
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(inner) = self.inner.take() {
            inner.stop().await?;
        }
        Ok(())
    }

    pub fn is_started(&self) -> bool {
        self.inner.is_some()
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

    fn inner(&self) -> Result<&Arc<TransportInner>> {
        self.inner.as_ref().ok_or(Error::TransportNotStarted)
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
    matches!(
        event.event_type.as_str(),
        "agent_end" | "turn_end" | "message_end" | "error"
    )
}

struct TransportInner {
    config: Config,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>,
    next_id: AtomicU64,
    events: broadcast::Sender<SdkEvent>,
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
            .stderr(Stdio::piped());
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
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            events,
        });
        Self::spawn_stdout_reader(inner.clone(), stdout);
        if let Some(stderr) = stderr {
            Self::spawn_stderr_reader(inner.clone(), stderr);
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
        Ok(())
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&message)?;

        {
            let mut stdin = self.stdin.lock().await;
            let stdin = stdin.as_mut().ok_or(Error::ChannelClosed)?;
            stdin.write_all(line.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        match time::timeout(self.config.timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Error::ChannelClosed),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(Error::RequestTimeout(method.to_string()))
            }
        }
    }

    fn spawn_stdout_reader(inner: Arc<Self>, stdout: tokio::process::ChildStdout) {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Err(error) = inner.handle_line(&line).await {
                    let _ = inner.events.send(SdkEvent::new(
                        "error",
                        json!({ "type": "error", "message": error.to_string() }),
                    ));
                }
            }
        });
    }

    fn spawn_stderr_reader(inner: Arc<Self>, stderr: tokio::process::ChildStderr) {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if inner.config.debug {
                    eprintln!("[autohand] {line}");
                }
            }
        });
    }

    async fn handle_line(&self, line: &str) -> Result<()> {
        let value: Value = serde_json::from_str(line)?;
        if let Some(id) = value.get("id").and_then(Value::as_u64) {
            if let Some(tx) = self.pending.lock().await.remove(&id) {
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
    use std::{fs, os::unix::fs::PermissionsExt};

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

        let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
        sdk.start().await.unwrap();
        let mut events = sdk
            .stream_prompt("hello", PromptOptions::default())
            .await
            .unwrap();
        let mut text = String::new();
        let mut answered_permission = false;
        while let Some(event) = events.recv().await {
            let event = event.unwrap();
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
    }
}
