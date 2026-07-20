#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

use autohand_sdk::{Agent, AutohandSdk, Config};
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
