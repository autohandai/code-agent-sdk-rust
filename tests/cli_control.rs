#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

use autohand_sdk::{
    Agent, AutohandSdk, BrowserHandoffAttachParams, BrowserHandoffCreateParams, Config,
};
use serde_json::Value;
use tempfile::{tempdir, TempDir};

async fn fixture(result: &str) -> (TempDir, PathBuf, AutohandSdk) {
    let dir = tempdir().unwrap();
    let cli = dir.path().join("fake-autohand");
    let log = dir.path().join("requests.jsonl");
    fs::write(
        &cli,
        format!(
            r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "{}"
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.getState*) response='{{"ready":true}}' ;;
    *) response='{}' ;;
  esac
  printf '{{"jsonrpc":"2.0","id":%s,"result":%s}}\n' "$id" "$response"
done
"#,
            log.display(),
            result
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&cli).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cli, permissions).unwrap();

    let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
    sdk.start().await.unwrap();
    (dir, log, sdk)
}

fn sole_control_request(log: &PathBuf) -> Value {
    fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|request| request["method"] != "autohand.getState")
        .unwrap()
}

#[tokio::test]
async fn reset_uses_empty_params_and_decodes_the_new_session() {
    let (_dir, log, sdk) = fixture(r#"{"sessionId":"session-new"}"#).await;
    let mut agent = Agent::from_sdk(sdk);

    let result = agent.reset().await.unwrap();
    assert_eq!(result.session_id, "session-new");
    agent.close().await.unwrap();

    let request = sole_control_request(&log);
    assert_eq!(request["method"], "autohand.reset");
    assert_eq!(request["params"], serde_json::json!({}));
}

#[tokio::test]
async fn browser_handoff_create_preserves_camel_case_and_decodes_all_fields() {
    let (_dir, log, sdk) = fixture(
        r#"{"token":"handoff-token","sessionId":"session-1","workspaceRoot":"/workspace","createdAt":"2026-07-20T01:00:00Z","expiresAt":"2026-07-20T01:05:00Z","url":"chrome-extension://ext/continue"}"#,
    )
    .await;
    let mut agent = Agent::from_sdk(sdk);

    let result = agent
        .create_browser_handoff(BrowserHandoffCreateParams {
            extension_id: Some("ext-id".into()),
            install_url: Some("https://example.test/install".into()),
        })
        .await
        .unwrap();
    assert_eq!(result.token, "handoff-token");
    assert_eq!(result.session_id, "session-1");
    assert_eq!(result.workspace_root, "/workspace");
    assert_eq!(result.created_at, "2026-07-20T01:00:00Z");
    assert_eq!(result.expires_at, "2026-07-20T01:05:00Z");
    assert_eq!(result.url, "chrome-extension://ext/continue");
    agent.close().await.unwrap();

    let request = sole_control_request(&log);
    assert_eq!(request["method"], "autohand.browserHandoff.create");
    assert_eq!(
        request["params"],
        serde_json::json!({
            "extensionId": "ext-id",
            "installUrl": "https://example.test/install"
        })
    );
}

#[tokio::test]
async fn browser_handoff_attach_sends_the_token_and_decodes_optional_session_fields() {
    let (_dir, log, sdk) = fixture(
        r#"{"success":true,"sessionId":"session-1","workspaceRoot":"/workspace","messageCount":7}"#,
    )
    .await;
    let mut agent = Agent::from_sdk(sdk);

    let result = agent
        .attach_browser_handoff(BrowserHandoffAttachParams {
            token: "handoff-token".into(),
        })
        .await
        .unwrap();
    assert!(result.success);
    assert_eq!(result.session_id.as_deref(), Some("session-1"));
    assert_eq!(result.workspace_root.as_deref(), Some("/workspace"));
    assert_eq!(result.message_count, Some(7));
    agent.close().await.unwrap();

    let request = sole_control_request(&log);
    assert_eq!(request["method"], "autohand.browserHandoff.attach");
    assert_eq!(
        request["params"],
        serde_json::json!({"token": "handoff-token"})
    );
}
