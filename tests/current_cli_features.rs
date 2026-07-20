#![cfg(unix)]

use autohand_sdk::{
    AutohandSdk, ChangesDecisionParams, Config, Error, GetHistoryParams, SessionHistoryStatus,
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
