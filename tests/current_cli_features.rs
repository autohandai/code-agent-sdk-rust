#![cfg(unix)]

use autohand_sdk::{
    AutohandSdk, ChangesDecisionParams, Config, Error, GetHistoryParams, LearnRecommendParams,
    LearningAuditStatus, LearningUpdateStatus, McpInputSchema, McpInputSchemaType,
    McpInvocationResponseParams, McpSetVsCodeToolsParams, McpVsCodeTool, SessionHistoryStatus,
    SessionLookupResult, YoloSetParams,
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
