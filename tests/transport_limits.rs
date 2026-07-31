#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

use autohand_sdk::{Agent, AutohandSdk, Config, Error, PromptOptions};
use tempfile::tempdir;

fn write_cli(path: &std::path::Path, body: &str) {
    fs::write(path, body).expect("write fixture CLI");
    let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make fixture executable");
}

#[tokio::test]
async fn stdout_is_bounded_before_a_line_can_allocate_without_limit() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
printf '%0200d\n' 0
while IFS= read -r ignored; do :; done
"#,
    );
    let mut config = Config::default().with_cli_path(cli);
    config.max_stdout_bytes = 64;
    let result = Agent::create(config).await;
    assert!(matches!(
        result,
        Err(Error::OutputLimitExceeded {
            stream: "stdout",
            limit: 64
        })
    ));
}

#[tokio::test]
async fn stderr_is_bounded_even_when_debug_output_is_disabled() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
printf '%0200d\n' 0 >&2
while IFS= read -r ignored; do :; done
"#,
    );
    let mut config = Config::default().with_cli_path(cli);
    config.max_stderr_bytes = 64;
    let result = Agent::create(config).await;
    assert!(matches!(
        result,
        Err(Error::OutputLimitExceeded {
            stream: "stderr",
            limit: 64
        })
    ));
}

#[tokio::test]
async fn run_collection_stops_at_the_configured_event_count() {
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
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.messageUpdate","params":{"type":"message_update","delta":"one"}}'
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.agentEnd","params":{"type":"agent_end","reason":"completed"}}'
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ready":true}}\n' "$id"
      ;;
  esac
done
"#,
    );
    let mut config = Config::default().with_cli_path(cli);
    config.max_events = 1;
    let agent = Agent::create(config).await.expect("start fixture");
    assert!(matches!(
        agent.run("answer").await,
        Err(Error::EventLimitExceeded { limit: 1 })
    ));
}

#[tokio::test]
async fn run_collection_stops_at_the_configured_event_byte_limit() {
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
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.messageUpdate","params":{"type":"message_update","delta":"this event is deliberately larger than the configured byte limit"}}'
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.agentEnd","params":{"type":"agent_end","reason":"completed"}}'
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ready":true}}\n' "$id"
      ;;
  esac
done
"#,
    );
    let mut config = Config::default().with_cli_path(cli);
    config.max_event_bytes = 32;
    let agent = Agent::create(config).await.expect("start fixture");
    assert!(matches!(
        agent.run("answer").await,
        Err(Error::OutputLimitExceeded {
            stream: "captured events",
            limit: 32
        })
    ));
}

#[tokio::test]
async fn malformed_json_rpc_framing_fails_closed() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    write_cli(
        &cli,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  printf '{"id":%s,"result":{"ready":true}}\n' "$id"
done
"#,
    );
    assert!(matches!(
        Agent::create(Config::default().with_cli_path(cli)).await,
        Err(Error::Protocol(message)) if message.contains("version 2.0")
    ));
}

#[tokio::test]
async fn event_broadcast_lag_is_reported_instead_of_silently_skipped() {
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
      i=0
      while [ "$i" -lt 900 ]; do
        printf '{"jsonrpc":"2.0","method":"autohand.messageUpdate","params":{"type":"message_update","delta":"%s"}}\n' "$i"
        i=$((i + 1))
      done
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.agentEnd","params":{"type":"agent_end","reason":"completed"}}'
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ready":true}}\n' "$id"
      ;;
  esac
done
"#,
    );
    let mut sdk = AutohandSdk::new(Config::default().with_cli_path(cli));
    sdk.start().await.expect("start fixture");
    let mut events = sdk
        .stream_prompt("answer", PromptOptions::default())
        .await
        .expect("start event stream");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let reported = tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(event) = events.recv().await {
            if matches!(event, Err(Error::EventStreamLagged { .. })) {
                return true;
            }
        }
        false
    })
    .await
    .expect("lag result should not hang");
    assert!(reported, "broadcast lag must be a visible terminal error");
}
