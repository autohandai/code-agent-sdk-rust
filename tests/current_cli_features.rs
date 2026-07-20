#![cfg(unix)]

use autohand_sdk::{AutohandSdk, Config, Error};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};
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
