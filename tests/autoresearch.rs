#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt};

use autohand_sdk::{
    AutohandSdk, AutoresearchCompareParams, AutoresearchEvaluatorMode, AutoresearchHookEvent,
    AutoresearchPinParams, AutoresearchPruneParams, AutoresearchReplayParams,
    AutoresearchRescoreParams, AutoresearchStartParams, Config,
};
use tempfile::tempdir;

#[tokio::test]
async fn routes_typed_autoresearch_ledger_methods() {
    let dir = tempdir().expect("temporary directory");
    let cli = dir.path().join("fake-autohand");
    fs::write(
        &cli,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.autoresearch.start*)
      result='{"success":true,"instruction":"Run the next experiment"}' ;;
    *autohand.autoresearch.status*)
      result='{"success":true,"active":true,"statusText":"Auto-research active","runsLogged":2}' ;;
    *autohand.autoresearch.stop*)
      result='{"success":true,"active":false}' ;;
    *autohand.autoresearch.history*)
      result='{"success":true,"attempts":[]}' ;;
    *autohand.autoresearch.replay*)
      result='{"success":true,"attemptId":"attempt-1","evaluatorMode":"original"}' ;;
    *autohand.autoresearch.rescore*)
      result='{"success":true,"decisions":[]}' ;;
    *autohand.autoresearch.compare*)
      result='{"success":true}' ;;
    *autohand.autoresearch.pareto*)
      result='{"success":true,"attemptIds":[]}' ;;
    *autohand.autoresearch.pin*)
      result='{"success":true,"attemptId":"attempt-1","pinned":true}' ;;
    *autohand.autoresearch.prune*)
      result='{"success":true,"applied":false,"candidates":[],"bytesFreed":0,"remainingBytes":0}' ;;
    *)
      result='{"success":false,"error":"unexpected method"}' ;;
  esac
  printf '{"jsonrpc":"2.0","id":%s,"result":%s}\n' "$id" "$result"
done
"#,
    )
    .expect("write fake CLI");
    let mut permissions = fs::metadata(&cli).expect("fake CLI metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cli, permissions).expect("make fake CLI executable");

    let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
    sdk.start().await.expect("start SDK");

    let started = sdk
        .start_autoresearch(AutoresearchStartParams {
            objective: "Reduce latency".to_string(),
            metric_name: Some("p95_ms".to_string()),
            measure_command: Some("cargo test".to_string()),
            ..AutoresearchStartParams::default()
        })
        .await
        .expect("start autoresearch");
    assert!(started.success);
    assert_eq!(
        started.instruction.as_deref(),
        Some("Run the next experiment")
    );

    let status = sdk
        .get_autoresearch_status()
        .await
        .expect("autoresearch status");
    assert!(status.active);
    assert_eq!(status.runs_logged, 2);
    sdk.stop_autoresearch().await.expect("stop autoresearch");
    sdk.get_autoresearch_history()
        .await
        .expect("autoresearch history");
    sdk.replay_autoresearch(AutoresearchReplayParams {
        attempt_id: "attempt-1".to_string(),
        evaluator: Some(AutoresearchEvaluatorMode::Original),
    })
    .await
    .expect("replay autoresearch");
    sdk.rescore_autoresearch(AutoresearchRescoreParams::attempt("attempt-1"))
        .await
        .expect("rescore autoresearch");
    sdk.compare_autoresearch(AutoresearchCompareParams {
        left_attempt_id: "attempt-1".to_string(),
        right_attempt_id: "attempt-2".to_string(),
    })
    .await
    .expect("compare autoresearch");
    sdk.get_autoresearch_pareto()
        .await
        .expect("autoresearch pareto");
    sdk.pin_autoresearch(AutoresearchPinParams {
        attempt_id: "attempt-1".to_string(),
        pinned: true,
    })
    .await
    .expect("pin autoresearch");
    sdk.prune_autoresearch(AutoresearchPruneParams {
        dry_run: Some(true),
        yes: None,
    })
    .await
    .expect("prune autoresearch");

    sdk.stop().await.expect("stop SDK");
}

#[test]
fn rescore_params_serialize_only_valid_states() {
    assert_eq!(
        serde_json::to_value(AutoresearchRescoreParams::attempt("attempt-1"))
            .expect("serialize attempt"),
        serde_json::json!({ "attemptId": "attempt-1" })
    );
    assert_eq!(
        serde_json::to_value(AutoresearchRescoreParams::all()).expect("serialize all"),
        serde_json::json!({ "all": true })
    );
}

#[test]
fn hook_events_use_canonical_cli_names() {
    assert_eq!(
        serde_json::to_string(&AutoresearchHookEvent::Decision).expect("serialize decision hook"),
        r#""autoresearch:decision""#
    );
    assert_eq!(
        serde_json::to_string(&AutoresearchHookEvent::Error).expect("serialize error hook"),
        r#""autoresearch:error""#
    );
}
