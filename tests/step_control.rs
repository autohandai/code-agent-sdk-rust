#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};

use autohand_sdk::{
    has_tool_call, is_step_count, Agent, AutohandSdk, Config, Error, PromptOptions, RunStatus,
    StopCondition,
};
use tempfile::{tempdir, TempDir};
use tokio::{sync::Notify, time::timeout};

async fn fixture() -> (Agent, TempDir) {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("autohand");
    fs::write(&cli, r#"#!/bin/sh
step() {
  if [ "$mode" = malformed ]; then
    printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.stepEnd","params":{"stepId":"bad"}}'
  else
    printf '{"jsonrpc":"2.0","method":"autohand.stepEnd","params":{"stepId":"%s","step":{"stepNumber":%s,"toolCalls":[{"tool":"read_file","args":{"path":"evidence.txt"}}],"toolResults":[{"tool":"read_file","success":true,"output":"saved evidence"}]},"timestamp":"now"}}\n' "$count" "$count"
  fi
}
finish() {
  printf '{"jsonrpc":"2.0","method":"autohand.turnEnd","params":{"turnId":"turn-1","reason":"%s","timestamp":"now"}}\n' "$1"
  active=''
}
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$AUTOHAND_TEST_LOG"
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.prompt*)
      case "$line" in
        *'"message":""'*) printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32602,"message":"empty prompt"}}\n' "$id"; continue ;;
      esac
      if [ -n "$active" ]; then
        printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32000,"message":"overlapping prompt"}}\n' "$id"
        continue
      fi
      active=1
      mode=''
      case "$line" in
        *malformed*) mode=malformed ;;
        *rejected*) mode=rejected ;;
      esac
      printf '{"jsonrpc":"2.0","id":%s,"result":{"success":true}}\n' "$id"
      printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.turnStart","params":{"turnId":"turn-1"}}'
      case "$line" in
        *'"stopWhen":{"mode":"host"}'*)
          count=1; step
          case "$line" in
            *duplicate*) count=2; step ;;
            *flood*|*overflow*)
              i=0
              limit=300
              case "$line" in *overflow*) limit=900 ;; esac
              while [ "$i" -lt "$limit" ]; do
                printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.messageUpdate","params":{"delta":"x"}}'
                i=$((i + 1))
              done
              touch "$AUTOHAND_TEST_LOG.flooded"
              ;;
          esac ;;
        *hold*) : ;;
        *) printf '%s\n' '{"jsonrpc":"2.0","method":"autohand.messageEnd","params":{"content":"continued"}}'; finish completed ;;
      esac ;;
    *autohand.stepDecision*)
      if [ "$mode" = rejected ]; then
        printf '{"jsonrpc":"2.0","id":%s,"result":{"success":false}}\n' "$id"
        continue
      fi
      printf '{"jsonrpc":"2.0","id":%s,"result":{"success":true}}\n' "$id"
      case "$line" in
        *'"stop":true'*) finish stop_condition ;;
        *) count=$((count + 1)); step ;;
      esac ;;
    *autohand.abort*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"success":true}}\n' "$id"
      finish aborted ;;
    *) printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id" ;;
  esac
done
"#).expect("write fixture");
    fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).expect("executable");
    let mut config = Config::default().with_cli_path(cli);
    config.env.insert(
        "AUTOHAND_TEST_LOG".into(),
        directory
            .path()
            .join("requests.jsonl")
            .display()
            .to_string(),
    );
    config.timeout = Duration::from_secs(3);
    (
        Agent::create(config).await.expect("start fixture"),
        directory,
    )
}

fn options(condition: StopCondition) -> PromptOptions {
    PromptOptions {
        stop_when: vec![condition],
        ..PromptOptions::default()
    }
}

#[tokio::test]
async fn stops_at_persisted_steps_and_continues_after_turn_end_only() {
    let (mut agent, _directory) = fixture().await;
    let mut run = agent
        .send_with_options("inspect", options(is_step_count(2).unwrap()))
        .await
        .unwrap();
    let result = timeout(Duration::from_secs(3), run.wait())
        .await
        .expect("turn completes")
        .unwrap();
    assert_eq!(result.status, RunStatus::Stopped);
    assert_eq!(result.steps.len(), 2);
    assert_eq!(
        result.steps[0].tool_results[0].output.as_deref(),
        Some("saved evidence")
    );
    assert_eq!(run.wait().await.unwrap().status, RunStatus::Stopped);
    let continued = timeout(Duration::from_secs(3), agent.run("continue"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(continued.status, RunStatus::Completed);
    assert_eq!(continued.text, "continued");
    agent.close().await.unwrap();
}

#[tokio::test]
async fn predicate_failure_stops_before_becoming_an_error() {
    let (mut agent, directory) = fixture().await;
    let fail =
        StopCondition::new(|_| async { Err(Error::InvalidInput("predicate failed".into())) });
    let result = timeout(
        Duration::from_secs(3),
        agent.run_with_options("inspect", options(fail)),
    )
    .await
    .unwrap();
    assert!(matches!(result, Err(Error::InvalidInput(message)) if message == "predicate failed"));
    let requests = fs::read_to_string(directory.path().join("requests.jsonl")).unwrap();
    assert!(requests.contains("\"stop\":true"));
    assert!(
        !requests.contains("autohand.abort"),
        "an accepted stop must be drained without sending a second terminal request"
    );
    assert_eq!(
        agent.run("continue").await.unwrap().status,
        RunStatus::Completed
    );
    agent.close().await.unwrap();
}

#[tokio::test]
async fn cancelling_queued_run_does_not_kill_active_process() {
    let (mut agent, _directory) = fixture().await;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let condition = StopCondition::new({
        let entered = entered.clone();
        let release = release.clone();
        move |_| {
            let entered = entered.clone();
            let release = release.clone();
            async move {
                entered.notify_one();
                release.notified().await;
                Ok(true)
            }
        }
    });
    let mut active = agent
        .send_with_options("inspect", options(condition))
        .await
        .unwrap();
    timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    let mut queued = agent.send("queued").await.unwrap();
    queued.abort().await.unwrap();
    assert_eq!(
        timeout(Duration::from_secs(3), queued.wait())
            .await
            .unwrap()
            .unwrap()
            .status,
        RunStatus::Cancelled
    );
    release.notify_one();
    assert_eq!(
        timeout(Duration::from_secs(3), active.wait())
            .await
            .unwrap()
            .unwrap()
            .status,
        RunStatus::Stopped
    );
    assert_eq!(
        agent.run("continue").await.unwrap().status,
        RunStatus::Completed
    );
    agent.close().await.unwrap();
}

#[tokio::test]
async fn protocol_failures_settle_before_reusing_session() {
    for message in ["malformed", "rejected"] {
        let (mut agent, _directory) = fixture().await;
        let result = timeout(
            Duration::from_secs(3),
            agent.run_with_options(message, options(is_step_count(1).unwrap())),
        )
        .await
        .unwrap();
        assert!(result.is_err(), "{message}: {result:?}");
        assert_eq!(
            agent.run("continue").await.unwrap().status,
            RunStatus::Completed
        );
        agent.close().await.unwrap();
    }
}

#[test]
fn helpers_reject_invalid_boundaries() {
    assert!(is_step_count(0).is_err());
    assert!(has_tool_call(" ").is_err());
}

#[tokio::test]
async fn rejected_prompt_does_not_abort_or_retire_the_session() {
    let (mut agent, directory) = fixture().await;
    assert!(matches!(
        agent.run("").await,
        Err(Error::Rpc { code: -32602, .. })
    ));
    assert!(!fs::read_to_string(directory.path().join("requests.jsonl"))
        .unwrap()
        .contains("autohand.abort"));
    assert_eq!(
        agent.run("continue").await.unwrap().status,
        RunStatus::Completed
    );
    agent.close().await.unwrap();
}

#[tokio::test]
async fn dropping_an_unsubmitted_raw_stream_keeps_the_cli_usable() {
    let (mut unused_agent, directory) = fixture().await;
    unused_agent.close().await.unwrap();
    let log = directory.path().join("raw.jsonl");
    let mut config = Config::default().with_cli_path(directory.path().join("autohand"));
    config
        .env
        .insert("AUTOHAND_TEST_LOG".into(), log.display().to_string());
    let mut sdk = AutohandSdk::new(config);
    sdk.start().await.unwrap();
    let unused = sdk
        .stream_prompt("unused", PromptOptions::default())
        .await
        .unwrap();
    drop(unused);
    let result = timeout(
        Duration::from_secs(3),
        sdk.prompt("continue", PromptOptions::default()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result, serde_json::json!({"success": true}));
    assert!(!fs::read_to_string(log).unwrap().contains("autohand.abort"));
    sdk.stop().await.unwrap();
}

#[tokio::test]
async fn pending_error_delivery_does_not_hold_the_next_turn() {
    let (mut unused_agent, directory) = fixture().await;
    unused_agent.close().await.unwrap();
    let log = directory.path().join("overflow.jsonl");
    let mut config = Config::default().with_cli_path(directory.path().join("autohand"));
    config
        .env
        .insert("AUTOHAND_TEST_LOG".into(), log.display().to_string());
    let mut sdk = AutohandSdk::new(config);
    sdk.start().await.unwrap();
    let mut first = sdk
        .stream_prompt(
            "overflow",
            options(StopCondition::new(|_| std::future::pending())),
        )
        .await
        .unwrap();
    timeout(Duration::from_secs(3), async {
        while !directory.path().join("overflow.jsonl.flooded").exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    first.recv().await.unwrap().unwrap();
    let next = timeout(
        Duration::from_secs(3),
        sdk.prompt("continue", PromptOptions::default()),
    )
    .await
    .expect("settled error delivery must not retain the turn lock")
    .unwrap();
    assert_eq!(next, serde_json::json!({"success": true}));
    assert_eq!(
        first.len(),
        256,
        "the old receiver remains full while the next turn completes"
    );
    drop(first);
    sdk.stop().await.unwrap();
}

struct DropSignal(Arc<Notify>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

#[tokio::test]
async fn abort_cancels_pending_predicate_even_when_forwarding_is_blocked() {
    for message in ["duplicate", "flood"] {
        let (mut agent, directory) = fixture().await;
        let entered = Arc::new(Notify::new());
        let dropped = Arc::new(Notify::new());
        let condition = StopCondition::new({
            let entered = entered.clone();
            let dropped = dropped.clone();
            move |_| {
                let guard = DropSignal(dropped.clone());
                let entered = entered.clone();
                async move {
                    let _guard = guard;
                    entered.notify_one();
                    std::future::pending().await
                }
            }
        });
        let mut run = agent
            .send_with_options(message, options(condition))
            .await
            .unwrap();
        timeout(Duration::from_secs(3), entered.notified())
            .await
            .unwrap();
        if message == "flood" {
            timeout(Duration::from_secs(3), async {
                while !directory.path().join("requests.jsonl.flooded").exists() {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
        }
        run.abort().await.unwrap();
        timeout(Duration::from_millis(500), dropped.notified())
            .await
            .expect("predicate task must be dropped while the unread Run remains alive");
        assert_eq!(
            timeout(Duration::from_secs(3), run.wait())
                .await
                .unwrap()
                .unwrap()
                .status,
            RunStatus::Cancelled
        );
        agent.close().await.unwrap();
    }
}

#[tokio::test]
async fn default_prompt_and_stream_share_the_same_turn_queue() {
    let (mut unused_agent, directory) = fixture().await;
    unused_agent.close().await.unwrap();
    let log = directory.path().join("mixed.jsonl");
    let mut config = Config::default().with_cli_path(directory.path().join("autohand"));
    config
        .env
        .insert("AUTOHAND_TEST_LOG".into(), log.display().to_string());
    let mut sdk = AutohandSdk::new(config);
    sdk.start().await.unwrap();
    let first = tokio::spawn({
        let sdk = sdk.clone();
        async move { sdk.prompt("hold", PromptOptions::default()).await }
    });
    timeout(Duration::from_secs(3), async {
        while !fs::read_to_string(&log).unwrap().contains("hold") {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let mut second = sdk
        .stream_prompt("second", PromptOptions::default())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!fs::read_to_string(&log).unwrap().contains("second"));
    sdk.interrupt().await.unwrap();
    assert!(timeout(Duration::from_secs(3), first)
        .await
        .unwrap()
        .unwrap()
        .is_ok());
    let mut completed = false;
    while let Some(event) = timeout(Duration::from_secs(3), second.recv())
        .await
        .unwrap()
    {
        let event = event.unwrap();
        completed |= event.event_type == "turn_end" && event.raw["reason"] == "completed";
    }
    assert!(completed);
    sdk.stop().await.unwrap();
}
