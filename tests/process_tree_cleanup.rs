#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, time::Duration};

use autohand_sdk::{Agent, Config, Error, SetupOnlyProfile};
use tempfile::tempdir;

fn write_cli(path: &Path, descendant_pid: &Path) {
    fs::write(
        path,
        format!(
            r#"#!/bin/sh
sleep 60 &
printf '%s' "$!" > "{descendant_pid}"
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.getState*)
      printf '{{"jsonrpc":"2.0","id":%s,"result":{{"ready":true}}}}\n' "$id"
      ;;
    *)
      # Deliberately keep the request pending until the SDK terminates the tree.
      ;;
  esac
done
"#,
            descendant_pid = descendant_pid.display(),
        ),
    )
    .expect("write fixture CLI");
    let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make fixture executable");
}

fn write_setup_cli(path: &Path, descendant_pid: &Path) {
    fs::write(
        path,
        format!(
            r#"#!/bin/sh
sleep 60 &
printf '%s' "$!" > "{descendant_pid}"
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.login.begin*)
      printf '{{"jsonrpc":"2.0","id":%s,"result":{{"contractVersion":1,"sessionId":"0123456789abcdef0123456789abcdef","userCode":"ABCD-EFGH","verificationUriComplete":"https://autohand.ai/signin?continue=signed&user_code=ABCD-EFGH","expiresAtUnixMs":9999999999999,"pollAfterMs":1000}}}}\n' "$id"
      ;;
    *)
      ;;
  esac
done
"#,
            descendant_pid = descendant_pid.display(),
        ),
    )
    .expect("write setup fixture CLI");
    let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make fixture executable");
}

async fn read_pid(path: &Path) -> i32 {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(value) = fs::read_to_string(path) {
                if let Ok(pid) = value.parse::<i32>() {
                    break pid;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("descendant PID marker")
}

fn process_exists(pid: i32) -> bool {
    if unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    let output = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("inspect descendant state");
    let state = String::from_utf8_lossy(&output.stdout);
    !state.trim_start().starts_with('Z')
}

async fn assert_process_gone(pid: i32) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while process_exists(pid) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("descendant process should be reaped");
}

#[tokio::test]
async fn stop_terminates_the_complete_process_group() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let marker = directory.path().join("descendant.pid");
    write_cli(&cli, &marker);
    let mut sdk = autohand_sdk::AutohandSdk::new(Config::default().with_cli_path(cli));
    sdk.start().await.expect("start fixture");
    let pid = read_pid(&marker).await;
    assert!(process_exists(pid));
    sdk.stop().await.expect("stop fixture");
    assert_process_gone(pid).await;
}

#[tokio::test]
async fn request_timeout_terminates_the_complete_process_group() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let marker = directory.path().join("descendant.pid");
    write_cli(&cli, &marker);
    let mut config = Config::default().with_cli_path(cli);
    config.timeout = Duration::from_secs(3);
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start fixture");
    let pid = read_pid(&marker).await;
    assert!(matches!(
        sdk.request("autohand.neverReplies", serde_json::json!({}))
            .await,
        Err(Error::RequestTimeout(method)) if method == "autohand.neverReplies"
    ));
    assert_process_gone(pid).await;
}

#[tokio::test]
async fn abort_and_dropped_run_each_terminate_the_complete_process_group() {
    let first = tempdir().expect("first fixture directory");
    let first_cli = first.path().join("fake-autohand");
    let first_marker = first.path().join("descendant.pid");
    write_cli(&first_cli, &first_marker);
    let agent = Agent::create(Config::default().with_cli_path(first_cli))
        .await
        .expect("start first fixture");
    let run = agent.send("pending").await.expect("start pending run");
    let first_pid = read_pid(&first_marker).await;
    run.abort().await.expect("abort run");
    assert_process_gone(first_pid).await;

    let second = tempdir().expect("second fixture directory");
    let second_cli = second.path().join("fake-autohand");
    let second_marker = second.path().join("descendant.pid");
    write_cli(&second_cli, &second_marker);
    let agent = Agent::create(Config::default().with_cli_path(second_cli))
        .await
        .expect("start second fixture");
    let run = agent.send("pending").await.expect("start pending run");
    let second_pid = read_pid(&second_marker).await;
    drop(run);
    assert_process_gone(second_pid).await;
}

#[tokio::test]
async fn dropping_the_last_sdk_handle_terminates_descendants() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let marker = directory.path().join("descendant.pid");
    write_cli(&cli, &marker);
    let mut sdk = autohand_sdk::AutohandSdk::new(Config::default().with_cli_path(cli));
    sdk.start().await.expect("start fixture");
    let pid = read_pid(&marker).await;
    drop(sdk);
    assert_process_gone(pid).await;
}

#[tokio::test]
async fn dropping_an_unfinished_opaque_login_session_terminates_descendants() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let marker = directory.path().join("descendant.pid");
    write_setup_cli(&cli, &marker);
    let config = Config::default()
        .with_cli_path(cli)
        .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start setup fixture");
    let challenge = sdk
        .begin_autohand_login()
        .await
        .expect("begin setup fixture");
    let pid = read_pid(&marker).await;
    assert!(process_exists(pid));
    drop(challenge);
    assert_process_gone(pid).await;
    assert!(!sdk.is_started());
}
