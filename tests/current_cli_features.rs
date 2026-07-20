#![cfg(unix)]

use autohand_sdk::{
    AutohandSdk, ChangesDecisionParams, Config, Error, GetHistoryParams, LearnGenerateParams,
    LearnRecommendParams, LearningAuditStatus, LearningUpdateStatus, McpInputSchema,
    McpInputSchemaType, McpInvocationResponseParams, McpSetVsCodeToolsParams, McpVsCodeTool,
    SessionHistoryStatus, SessionLookupResult, SkillGenerationScope, TokenUsageStatus,
    ToolRegistrySource, YoloSetParams,
};
use std::{fs, num::NonZeroU64, os::unix::fs::PermissionsExt, path::PathBuf};
use tempfile::{tempdir, TempDir};

struct CurrentCliFixture {
    sdk: AutohandSdk,
    _directory: TempDir,
    log_path: PathBuf,
}

impl CurrentCliFixture {
    async fn start(result: &str, notification: &str) -> Self {
        let directory = tempdir().expect("fixture directory");
        let cli = directory.path().join("fake-autohand");
        let log_path = directory.path().join("requests.log");
        fs::write(
            &cli,
            r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$AUTOHAND_TEST_LOG"
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.getState*) response='{}' ;;
    *autohand.prompt*)
      if [ -n "$AUTOHAND_TEST_NOTIFICATION" ]; then
        printf '%s\n' "$AUTOHAND_TEST_NOTIFICATION"
        printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.agentEnd","params":{"sessionId":"fixture","reason":"completed","timestamp":"now"}}'
      fi
      response='{"success":true}'
      ;;
    *) response="$AUTOHAND_TEST_RESULT" ;;
  esac
  printf '{"jsonrpc":"2.0","id":%s,"result":%s}\n' "$id" "$response"
done
"#,
        )
        .expect("write fake CLI");
        let mut permissions = fs::metadata(&cli).expect("fake CLI metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&cli, permissions).expect("make fake CLI executable");

        let mut config = Config::default().with_cli_path(&cli);
        config
            .env
            .insert("AUTOHAND_TEST_LOG".into(), log_path.display().to_string());
        config
            .env
            .insert("AUTOHAND_TEST_RESULT".into(), result.into());
        config
            .env
            .insert("AUTOHAND_TEST_NOTIFICATION".into(), notification.into());
        let mut sdk = AutohandSdk::new(config);
        sdk.start().await.expect("start fixture SDK");
        Self {
            sdk,
            _directory: directory,
            log_path,
        }
    }

    fn assert_request(&self, method: &str, params: &[&str]) {
        let log = fs::read_to_string(&self.log_path).expect("request log");
        assert!(
            log.contains(&format!(r#""method":"{method}""#)),
            "request log does not contain {method}: {log}"
        );
        for param in params {
            assert!(
                log.contains(param),
                "request log does not contain {param}: {log}"
            );
        }
    }
}

#[tokio::test]
async fn acknowledges_permission_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, "").await;
    let result = fixture
        .sdk
        .acknowledge_permission("permission-1")
        .await
        .expect("acknowledge permission");
    assert!(result.success);
    fixture.assert_request(
        "autohand.permissionAcknowledged",
        &[r#""requestId":"permission-1""#],
    );
    assert!(matches!(
        fixture.sdk.acknowledge_permission("  ").await,
        Err(Error::InvalidInput(_))
    ));
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn responds_to_directory_access_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, "").await;
    let result = fixture
        .sdk
        .respond_to_directory_access("directory-1", false)
        .await
        .expect("respond to directory access");
    assert!(result.success);
    fixture.assert_request(
        "autohand.directoryAccessResponse",
        &[r#""requestId":"directory-1""#, r#""granted":false"#],
    );
    assert!(matches!(
        fixture.sdk.respond_to_directory_access("", true).await,
        Err(Error::InvalidInput(_))
    ));
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn acknowledges_directory_access_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, "").await;
    let result = fixture
        .sdk
        .acknowledge_directory_access("directory-2")
        .await
        .expect("acknowledge directory access");
    assert!(result.success);
    fixture.assert_request(
        "autohand.directoryAccessAcknowledged",
        &[r#""requestId":"directory-2""#],
    );
    assert!(matches!(
        fixture.sdk.acknowledge_directory_access("\t").await,
        Err(Error::InvalidInput(_))
    ));
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn decides_multi_file_changes_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(
        r#"{"success":true,"appliedCount":1,"skippedCount":2,"errors":[]}"#,
        "",
    )
    .await;
    let result = fixture
        .sdk
        .decide_changes(ChangesDecisionParams::AcceptSelected {
            batch_id: "batch-1".into(),
            selected_change_ids: vec!["change-2".into()],
        })
        .await
        .expect("decide changes");
    assert_eq!(result.applied_count, 1);
    assert_eq!(result.skipped_count, 2);
    fixture.assert_request(
        "autohand.changesDecision",
        &[
            r#""action":"accept_selected""#,
            r#""batchId":"batch-1""#,
            r#""selectedChangeIds":["change-2"]"#,
        ],
    );
    assert!(matches!(
        fixture
            .sdk
            .decide_changes(ChangesDecisionParams::AcceptSelected {
                batch_id: "batch-1".into(),
                selected_change_ids: Vec::new(),
            })
            .await,
        Err(Error::InvalidInput(_))
    ));
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn gets_session_history_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(
        r#"{"sessions":[{"sessionId":"session-1","createdAt":"now","lastActiveAt":"later","projectName":"tin","model":"gpt-5","messageCount":4,"status":"completed"}],"currentPage":2,"totalPages":3,"totalItems":5}"#,
        "",
    )
    .await;
    let result = fixture
        .sdk
        .get_history(GetHistoryParams {
            page: NonZeroU64::new(2),
            page_size: NonZeroU64::new(1),
        })
        .await
        .expect("get session history");
    assert_eq!(result.total_items, 5);
    assert_eq!(result.sessions[0].status, SessionHistoryStatus::Completed);
    fixture.assert_request("autohand.getHistory", &[r#""page":2"#, r#""pageSize":1"#]);
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn gets_typed_session_details_and_failures_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(
        r#"{"success":true,"sessionId":"session-1","projectName":"tin","model":"gpt-5","messageCount":1,"status":"completed","createdAt":"now","lastActiveAt":"later","messages":[{"id":"message-1","role":"assistant","content":"done","timestamp":"later"}],"workspaceRoot":"/workspace"}"#,
        "",
    )
    .await;
    let result = fixture
        .sdk
        .get_session("session-1")
        .await
        .expect("get session details");
    match result {
        SessionLookupResult::Success(details) => assert_eq!(details.messages.len(), 1),
        SessionLookupResult::Failure { error } => panic!("unexpected failure: {error:?}"),
    }
    fixture.assert_request("autohand.getSession", &[r#""sessionId":"session-1""#]);
    fixture.sdk.stop().await.expect("stop fixture SDK");

    let mut missing =
        CurrentCliFixture::start(r#"{"success":false,"error":"not found"}"#, "").await;
    assert_eq!(
        missing
            .sdk
            .get_session("missing")
            .await
            .expect("failure result"),
        SessionLookupResult::Failure {
            error: Some("not found".into())
        }
    );
    missing.sdk.stop().await.expect("stop missing fixture");

    let mut malformed =
        CurrentCliFixture::start(r#"{"success":true,"sessionId":"partial"}"#, "").await;
    assert!(matches!(
        malformed.sdk.get_session("partial").await,
        Err(Error::Json(_))
    ));
    malformed.sdk.stop().await.expect("stop malformed fixture");
}

#[tokio::test]
async fn attaches_session_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(
        r#"{"success":true,"sessionId":"session-2","workspaceRoot":"/workspace","messageCount":6}"#,
        "",
    )
    .await;
    let result = fixture
        .sdk
        .attach_session("session-2")
        .await
        .expect("attach session");
    assert!(result.success);
    assert_eq!(result.message_count, Some(6));
    fixture.assert_request("autohand.session.attach", &[r#""sessionId":"session-2""#]);
    assert!(matches!(
        fixture.sdk.attach_session(" ").await,
        Err(Error::InvalidInput(_))
    ));
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn sets_timed_yolo_through_both_wire_methods() {
    let mut fixture = CurrentCliFixture::start(r#"{"success":true,"expiresIn":45}"#, "").await;
    let params = YoloSetParams {
        pattern: "*".into(),
        timeout_seconds: NonZeroU64::new(45),
    };
    let canonical = fixture
        .sdk
        .set_yolo(params.clone())
        .await
        .expect("set canonical YOLO mode");
    let alias = fixture
        .sdk
        .set_yolo_alias(params)
        .await
        .expect("set alias YOLO mode");
    assert_eq!(canonical.expires_in, Some(45));
    assert_eq!(alias.expires_in, Some(45));
    fixture.assert_request(
        "autohand.yoloSet",
        &[r#""pattern":"*""#, r#""timeoutSeconds":45"#],
    );
    fixture.assert_request("autohand.yolo.set", &[]);
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn registers_vscode_mcp_tools_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, "").await;
    let result = fixture
        .sdk
        .set_vscode_mcp_tools(McpSetVsCodeToolsParams {
            tools: vec![McpVsCodeTool {
                name: "issues".into(),
                description: "List issues".into(),
                server_name: "github".into(),
                input_schema: Some(McpInputSchema {
                    schema_type: McpInputSchemaType::Object,
                    properties: serde_json::from_value(serde_json::json!({
                        "state": {"type": "string"}
                    }))
                    .expect("object properties"),
                    required: vec!["state".into()],
                }),
            }],
        })
        .await
        .expect("register MCP tools");
    assert!(result.success);
    fixture.assert_request(
        "autohand.mcp.setVscodeTools",
        &[
            r#""serverName":"github""#,
            r#""inputSchema":{"properties""#,
            r#""type":"object""#,
            r#""required":["state"]"#,
        ],
    );
    assert!(matches!(
        fixture
            .sdk
            .set_vscode_mcp_tools(McpSetVsCodeToolsParams {
                tools: vec![McpVsCodeTool {
                    name: "broken".into(),
                    description: String::new(),
                    server_name: String::new(),
                    input_schema: None,
                }],
            })
            .await,
        Err(Error::InvalidInput(_))
    ));
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn responds_to_mcp_invocation_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, "").await;
    let result = fixture
        .sdk
        .respond_to_mcp_invocation(McpInvocationResponseParams::Failure {
            request_id: "invoke-1".into(),
            error: "tool unavailable".into(),
        })
        .await
        .expect("respond to MCP invocation");
    assert!(result.success);
    fixture.assert_request(
        "autohand.mcp.invokeResponse",
        &[
            r#""requestId":"invoke-1""#,
            r#""success":false"#,
            r#""error":"tool unavailable""#,
        ],
    );
    assert!(matches!(
        fixture
            .sdk
            .respond_to_mcp_invocation(McpInvocationResponseParams::Failure {
                request_id: "invoke-2".into(),
                error: String::new(),
            })
            .await,
        Err(Error::InvalidInput(_))
    ));

    fixture
        .sdk
        .respond_to_mcp_invocation(McpInvocationResponseParams::Success {
            request_id: "invoke-3".into(),
            result: None,
        })
        .await
        .expect("respond without a result body");
    let log = fs::read_to_string(&fixture.log_path).expect("request log");
    let request = log
        .lines()
        .find(|line| line.contains(r#""requestId":"invoke-3""#))
        .expect("success response request");
    assert!(!request.contains(r#""result""#));
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn recommends_project_learning_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(
        r#"{"success":true,"projectSummary":"Rust SDK","audit":[{"skill":"old","status":"outdated","reason":"stale"}],"recommendations":[{"slug":"rust-testing","score":0.95,"reason":"missing tests"}],"gapAnalysis":"Add integration coverage"}"#,
        "",
    )
    .await;
    let result = fixture
        .sdk
        .recommend_project_learning(LearnRecommendParams { deep: true })
        .await
        .expect("recommend project learning");
    assert_eq!(result.audit[0].status, LearningAuditStatus::Outdated);
    assert!(result.gap_analysis.is_some());
    fixture.assert_request("autohand.learn.recommend", &[r#""deep":true"#]);
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn updates_project_learning_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(
        r#"{"success":true,"updated":1,"unchanged":2,"results":[{"name":"testing","status":"updated"}]}"#,
        "",
    )
    .await;
    let result = fixture
        .sdk
        .update_project_learning()
        .await
        .expect("update project learning");
    assert_eq!(result.updated, 1);
    assert_eq!(result.results[0].status, LearningUpdateStatus::Updated);
    fixture.assert_request("autohand.learn.update", &[r#""params":{}"#]);
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn generates_project_skill_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(
        r#"{"success":true,"skillName":"release","skillPath":".autohand/skills/release"}"#,
        "",
    )
    .await;
    let result = fixture
        .sdk
        .generate_project_skill(LearnGenerateParams {
            scope: SkillGenerationScope::Project,
        })
        .await
        .expect("generate project skill");
    assert_eq!(result.skill_name.as_deref(), Some("release"));
    fixture.assert_request("autohand.learn.generate", &[r#""scope":"project""#]);
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn gets_tools_registry_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(
        r#"{"tools":[{"name":"read_file","description":"Read a file","requiresApproval":false,"source":"builtin"},{"name":"review","description":"Review code","source":"extension","scope":"project","extensionId":"quality"}],"diagnostics":[{"file":"broken.json","reason":"invalid schema"}]}"#,
        "",
    )
    .await;
    let result = fixture
        .sdk
        .get_tools_registry()
        .await
        .expect("get tools registry");
    assert_eq!(result.tools.len(), 2);
    assert_eq!(result.tools[1].source, ToolRegistrySource::Extension);
    assert_eq!(result.tools[0].requires_approval, Some(false));
    assert_eq!(result.diagnostics.len(), 1);
    fixture.assert_request("autohand.getToolsRegistry", &[r#""params":{}"#]);
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn sets_context_compaction_through_spawned_cli() {
    let mut fixture = CurrentCliFixture::start(r#"{"enabled":false}"#, "").await;
    let result = fixture
        .sdk
        .set_context_compact(false)
        .await
        .expect("set context compaction");
    assert!(!result.enabled);
    fixture.assert_request("autohand.setContextCompact", &[r#""enabled":false"#]);
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn streams_typed_automode_iteration_from_spawned_cli() {
    let notification = r#"{"jsonrpc":"2.0","method":"autohand.automode.iteration","params":{"sessionId":"auto-1","iteration":3,"actions":["edit","test"],"tokensUsed":321,"timestamp":"now"}}"#;
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, notification).await;
    let mut events = fixture
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start event stream");
    let event = events
        .recv()
        .await
        .expect("iteration event")
        .expect("valid SDK event");
    let typed = event
        .automode_iteration()
        .expect("auto-mode iteration kind")
        .expect("valid auto-mode iteration");
    assert_eq!(typed.iteration, 3);
    assert_eq!(typed.tokens_used, Some(321));
    fixture.sdk.stop().await.expect("stop fixture SDK");

    let malformed_notification =
        r#"{"jsonrpc":"2.0","method":"autohand.automode.iteration","params":{"sessionId":7}}"#;
    let mut malformed =
        CurrentCliFixture::start(r#"{"success":true}"#, malformed_notification).await;
    let mut malformed_events = malformed
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start malformed event stream");
    let malformed_event = malformed_events
        .recv()
        .await
        .expect("malformed event")
        .expect("raw SDK event remains available");
    assert!(malformed_event
        .automode_iteration()
        .expect("auto-mode iteration kind")
        .is_err());
    malformed.sdk.stop().await.expect("stop malformed fixture");
}

#[tokio::test]
async fn streams_typed_automode_completion_from_spawned_cli() {
    let notification = r#"{"jsonrpc":"2.0","method":"autohand.automode.complete","params":{"sessionId":"auto-1","iterations":4,"filesCreated":2,"filesModified":5,"timestamp":"now"}}"#;
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, notification).await;
    let mut events = fixture
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start event stream");
    let event = events
        .recv()
        .await
        .expect("completion event")
        .expect("valid SDK event");
    let typed = event
        .automode_complete()
        .expect("auto-mode completion kind")
        .expect("valid auto-mode completion");
    assert_eq!(typed.iterations, 4);
    assert_eq!(typed.files_modified, 5);
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn streams_typed_automode_error_from_spawned_cli() {
    let notification = r#"{"jsonrpc":"2.0","method":"autohand.automode.error","params":{"sessionId":"auto-1","error":"iteration failed","timestamp":"now"}}"#;
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, notification).await;
    let mut events = fixture
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start event stream");
    let event = events
        .recv()
        .await
        .expect("error event")
        .expect("valid SDK event");
    let typed = event
        .automode_error()
        .expect("auto-mode error kind")
        .expect("valid auto-mode error");
    assert_eq!(typed.error, "iteration failed");
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn streams_typed_pre_tool_hook_from_spawned_cli() {
    let notification = r#"{"jsonrpc":"2.0","method":"autohand.hook.preTool","params":{"toolId":"tool-1","toolName":"read_file","args":{"path":"README.md"},"timestamp":"now"}}"#;
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, notification).await;
    let mut events = fixture
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start event stream");
    let event = events
        .recv()
        .await
        .expect("pre-tool event")
        .expect("valid SDK event");
    let typed = event
        .hook_pre_tool()
        .expect("pre-tool hook kind")
        .expect("valid pre-tool hook");
    assert_eq!(typed.tool_name, "read_file");
    assert_eq!(
        typed.args.get("path").and_then(serde_json::Value::as_str),
        Some("README.md")
    );
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn streams_typed_post_tool_hook_from_spawned_cli() {
    let notification = r#"{"jsonrpc":"2.0","method":"autohand.hook.postTool","params":{"toolId":"tool-1","toolName":"read_file","success":true,"duration":12.5,"output":"contents","timestamp":"now"}}"#;
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, notification).await;
    let mut events = fixture
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start event stream");
    let event = events
        .recv()
        .await
        .expect("post-tool event")
        .expect("valid SDK event");
    let typed = event
        .hook_post_tool()
        .expect("post-tool hook kind")
        .expect("valid post-tool hook");
    assert!(typed.success);
    assert_eq!(typed.duration, 12.5);
    assert_eq!(typed.output.as_deref(), Some("contents"));
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn streams_typed_pre_prompt_hook_from_spawned_cli() {
    let notification = r#"{"jsonrpc":"2.0","method":"autohand.hook.prePrompt","params":{"instruction":"Review the SDK","mentionedFiles":["sdk.rs","event.rs"],"timestamp":"now"}}"#;
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, notification).await;
    let mut events = fixture
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start event stream");
    let event = events
        .recv()
        .await
        .expect("pre-prompt event")
        .expect("valid SDK event");
    let typed = event
        .hook_pre_prompt()
        .expect("pre-prompt hook kind")
        .expect("valid pre-prompt hook");
    assert_eq!(typed.instruction, "Review the SDK");
    assert_eq!(typed.mentioned_files.len(), 2);
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn streams_typed_post_response_hook_from_spawned_cli() {
    let notification = r#"{"jsonrpc":"2.0","method":"autohand.hook.postResponse","params":{"tokensUsed":0,"tokensUsageStatus":"unavailable","toolCallsCount":2,"duration":18.75,"timestamp":"now"}}"#;
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, notification).await;
    let mut events = fixture
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start event stream");
    let event = events
        .recv()
        .await
        .expect("post-response event")
        .expect("valid SDK event");
    let typed = event
        .hook_post_response()
        .expect("post-response hook kind")
        .expect("valid post-response hook");
    assert_eq!(typed.tool_calls_count, 2);
    assert_eq!(
        typed.tokens_usage_status,
        Some(TokenUsageStatus::Unavailable)
    );
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn streams_typed_mcp_invocation_request_from_spawned_cli() {
    let notification = r#"{"jsonrpc":"2.0","method":"autohand.mcp.invokeRequest","params":{"requestId":"invoke-7","toolName":"vscode__github__issues","args":{"state":"open"},"timestamp":"now"}}"#;
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, notification).await;
    let mut events = fixture
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start event stream");
    let event = events
        .recv()
        .await
        .expect("MCP invocation event")
        .expect("valid SDK event");
    let typed = event
        .mcp_invocation_request()
        .expect("MCP invocation request kind")
        .expect("valid MCP invocation request");
    assert_eq!(typed.request_id, "invoke-7");
    assert_eq!(
        typed.args.get("state").and_then(serde_json::Value::as_str),
        Some("open")
    );
    fixture.sdk.stop().await.expect("stop fixture SDK");
}

#[tokio::test]
async fn streams_typed_mcp_tools_changed_from_spawned_cli() {
    let notification = r#"{"jsonrpc":"2.0","method":"autohand.mcp.toolsChanged","params":{"tools":[{"name":"vscode__github__issues","description":"List issues","serverName":"github"}],"timestamp":"now"}}"#;
    let mut fixture = CurrentCliFixture::start(r#"{"success":true}"#, notification).await;
    let mut events = fixture
        .sdk
        .stream_prompt("emit", Default::default())
        .await
        .expect("start event stream");
    let event = events
        .recv()
        .await
        .expect("MCP tools event")
        .expect("valid SDK event");
    let typed = event
        .mcp_tools_changed()
        .expect("MCP tools changed kind")
        .expect("valid MCP tools changed event");
    assert_eq!(typed.tools.len(), 1);
    assert_eq!(typed.tools[0].server_name, "github");
    fixture.sdk.stop().await.expect("stop fixture SDK");
}
