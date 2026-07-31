#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt};

use autohand_sdk::{Agent, Config, Error, JsonRunOptions, RunStatus};
use tempfile::tempdir;

fn write_cli(path: &std::path::Path, body: &str) {
    fs::write(path, body).expect("write fixture CLI");
    let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make fixture executable");
}

#[tokio::test]
async fn terminal_error_is_a_failed_run() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.prompt*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"accepted":true}}\n' "$id"
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.messageEnd","params":{"type":"message_end","content":"{\"claimed\":true}"}}'
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.error","params":{"type":"error","message":"provider failed"}}'
      sleep 0.05
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.agentEnd","params":{"type":"agent_end","reason":"completed"}}'
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ready":true}}\n' "$id"
      ;;
  esac
done
"#,
    );

    let mut agent = Agent::create(Config::default().with_cli_path(cli))
        .await
        .expect("start fixture");
    let result = agent.run("answer").await.expect("collect terminal result");
    assert_eq!(result.status, RunStatus::Failed);
    let second = agent
        .run_json::<serde_json::Value>("answer", JsonRunOptions::default())
        .await;
    assert!(
        matches!(
            second,
            Err(Error::RunTerminated {
                status: RunStatus::Failed
            })
        ),
        "second failed run returned {second:?}"
    );
    agent.close().await.expect("stop fixture");
}

#[tokio::test]
async fn terminal_abort_is_a_cancelled_run() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.prompt*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"accepted":true}}\n' "$id"
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.agentEnd","params":{"type":"agent_end","reason":"aborted"}}'
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.agentEnd","params":{"type":"agent_end","reason":"completed"}}'
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ready":true}}\n' "$id"
      ;;
  esac
done
"#,
    );

    let mut agent = Agent::create(Config::default().with_cli_path(cli))
        .await
        .expect("start fixture");
    let result = agent.run("answer").await.expect("collect terminal result");
    assert_eq!(result.status, RunStatus::Cancelled);
    agent.close().await.expect("stop fixture");
}

#[tokio::test]
async fn accepted_prompt_without_a_terminal_event_is_an_error() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.prompt*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"accepted":true}}\n' "$id"
      exit 0
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ready":true}}\n' "$id"
      ;;
  esac
done
"#,
    );

    let agent = Agent::create(Config::default().with_cli_path(cli))
        .await
        .expect("start fixture");
    assert!(matches!(
        agent.run("answer").await,
        Err(Error::MissingTerminalEvent)
    ));
}

#[tokio::test]
async fn null_id_authentication_failure_is_typed() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
printf '%s\n' '{"jsonrpc":"2.0","id":null,"error":{"code":-32011,"message":"sign in required","data":{"kind":"authentication_required","stage":"startup","retryable":true,"providerId":"autohandai"}}}'
while IFS= read -r ignored; do :; done
"#,
    );

    let result = Agent::create(Config::default().with_cli_path(cli)).await;
    assert!(matches!(
        result,
        Err(Error::AuthenticationRequired {
            code: -32011,
            retryable: true,
            ..
        })
    ));
}

#[tokio::test]
async fn null_id_initialization_failure_is_typed() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
printf '%s\n' '{"jsonrpc":"2.0","id":null,"error":{"code":-32010,"message":"startup failed","data":{"kind":"initialization_failed","stage":"startup","retryable":false}}}'
while IFS= read -r ignored; do :; done
"#,
    );

    let result = Agent::create(Config::default().with_cli_path(cli)).await;
    assert!(matches!(
        result,
        Err(Error::InitializationFailed {
            code: -32010,
            retryable: false,
            stage,
            ..
        }) if stage == "startup"
    ));
}

#[tokio::test]
async fn unknown_null_id_error_is_not_mislabeled_as_initialization() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
printf '%s\n' '{"jsonrpc":"2.0","id":null,"error":{"code":-32999,"message":"unexpected unbound failure"}}'
while IFS= read -r ignored; do :; done
"#,
    );

    let result = Agent::create(Config::default().with_cli_path(cli)).await;
    assert!(matches!(
        result,
        Err(Error::Protocol(message)) if message.contains("unbound RPC error")
    ));
}

#[tokio::test]
async fn id_bound_runtime_authentication_failure_is_typed_separately_from_inspection() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.authRequired*)
      printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32011,"message":"sign in required","data":{"kind":"authentication_required","retryable":true,"providerId":"autohandai"}}}\n' "$id"
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ready":true}}\n' "$id"
      ;;
  esac
done
"#,
    );

    let mut sdk = autohand_sdk::AutohandSdk::new(Config::default().with_cli_path(cli));
    sdk.start().await.expect("start fixture");
    assert!(matches!(
        sdk.request("autohand.authRequired", serde_json::json!({}))
            .await,
        Err(Error::AuthenticationRequired {
            code: -32011,
            retryable: true,
            provider_id: Some(provider),
            ..
        }) if provider == "autohandai"
    ));
    sdk.stop().await.expect("stop fixture");
}
