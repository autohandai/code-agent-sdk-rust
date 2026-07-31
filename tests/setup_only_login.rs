#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt};

use autohand_sdk::{
    Config, Error, LoginProblemCode, LoginStatus, SetupOnlyProfile, AUTOHAND_LOGIN_CONTRACT_VERSION,
};
use tempfile::tempdir;

fn write_cli(
    path: &std::path::Path,
    args_log: &std::path::Path,
    invalid_uri: bool,
    invalid_status: bool,
) {
    let uri = if invalid_uri {
        "https://autohand.ai/signin?continue=signed&user_code=ABCD-EFGH&device_code=secret"
    } else {
        "https://autohand.ai/signin?continue=signed&user_code=ABCD-EFGH"
    };
    let status_extension = if invalid_status {
        r#","deviceCode":"must-remain-private""#
    } else {
        ""
    };
    fs::write(
        path,
        format!(
            r#"#!/bin/sh
printf '%s' "$*" > "{args_log}"
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.login.begin*)
      printf '{{"jsonrpc":"2.0","id":%s,"result":{{"contractVersion":1,"sessionId":"0123456789abcdef0123456789abcdef","userCode":"ABCD-EFGH","verificationUriComplete":"{uri}","expiresAtUnixMs":9999999999999,"pollAfterMs":1000}}}}\n' "$id"
      ;;
    *autohand.login.poll*)
      printf '{{"jsonrpc":"2.0","id":%s,"result":{{"contractVersion":1,"status":"pending","pollAfterMs":1000{status_extension}}}}}\n' "$id"
      ;;
    *autohand.login.cancel*)
      printf '{{"jsonrpc":"2.0","id":%s,"result":{{"contractVersion":1,"status":"cancelled"}}}}\n' "$id"
      ;;
    *)
      printf '{{"jsonrpc":"2.0","id":%s,"error":{{"code":-32601,"message":"method not allowed"}}}}\n' "$id"
      ;;
  esac
done
"#,
            args_log = args_log.display(),
        ),
    )
    .expect("write fixture CLI");
    let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make fixture executable");
}

#[tokio::test]
async fn setup_only_login_exposes_only_the_safe_opaque_challenge() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let args_log = directory.path().join("args.txt");
    write_cli(&cli, &args_log, false, false);
    let config = Config::default()
        .with_cli_path(cli)
        .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start setup-only fixture");

    let challenge = sdk
        .begin_autohand_login()
        .await
        .expect("begin login challenge");
    let session_debug = format!("{:?}", challenge.session);
    assert!(session_debug.contains("opaque"));
    assert!(!session_debug.contains("0123456789abcdef"));
    assert_eq!(challenge.user_code, "ABCD-EFGH");
    assert_eq!(
        challenge.verification_uri_complete,
        "https://autohand.ai/signin?continue=signed&user_code=ABCD-EFGH"
    );
    assert_eq!(AUTOHAND_LOGIN_CONTRACT_VERSION, 1);
    assert_eq!(
        sdk.poll_autohand_login(&challenge.session)
            .await
            .expect("poll login"),
        LoginStatus::Pending {
            poll_after_ms: 1000
        }
    );
    sdk.cancel_autohand_login(challenge.session)
        .await
        .expect("cancel login");
    let args = fs::read_to_string(args_log).expect("read args");
    assert!(args.contains("--setup-only"));
    assert!(args.contains("--restricted"));
    assert!(args.contains("--client-context blueprint"));
    assert!(!args.contains("--answer-only"));
    assert!(matches!(
        sdk.request("autohand.getState", serde_json::json!({}))
            .await,
        Err(Error::ProfileViolation {
            profile: "setup_only",
            ..
        })
    ));
    sdk.stop().await.expect("stop fixture");
}

#[tokio::test]
async fn challenge_rejects_extra_query_keys_that_could_expose_device_state() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let args_log = directory.path().join("args.txt");
    write_cli(&cli, &args_log, true, false);
    let config = Config::default()
        .with_cli_path(cli)
        .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start setup-only fixture");
    assert!(matches!(
        sdk.begin_autohand_login().await,
        Err(Error::Protocol(message)) if message.contains("unapproved query key")
    ));
    sdk.stop().await.expect("stop fixture");
}

#[tokio::test]
async fn setup_status_rejects_fields_outside_its_tagged_variant() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let args_log = directory.path().join("args.txt");
    write_cli(&cli, &args_log, false, true);
    let config = Config::default()
        .with_cli_path(cli)
        .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start setup-only fixture");
    let challenge = sdk
        .begin_autohand_login()
        .await
        .expect("begin login challenge");
    assert!(
        sdk.poll_autohand_login(&challenge.session).await.is_err(),
        "a pending status must reject private or status-incompatible fields"
    );
    sdk.stop().await.expect("stop fixture");
}

#[tokio::test]
async fn setup_rpc_failures_use_the_closed_safe_problem_type() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    fs::write(
        &cli,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32000,"message":"Autohand device authorization could not be initiated.","data":{"kind":"initiation_failed","stage":"request","retryable":true}}}\n' "$id"
done
"#,
    )
    .expect("write fixture CLI");
    let mut permissions = fs::metadata(&cli).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cli, permissions).expect("make fixture executable");
    let config = Config::default()
        .with_cli_path(cli)
        .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start setup-only fixture");
    assert!(matches!(
        sdk.begin_autohand_login().await,
        Err(Error::LoginFailed { problem })
            if problem.code == LoginProblemCode::InitiationFailed && problem.retryable
    ));
    sdk.stop().await.expect("stop fixture");
}

#[tokio::test]
async fn unknown_setup_rpc_failures_do_not_expose_raw_authorization_data() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    fs::write(
        &cli,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32099,"message":"provider body must not escape","data":{"kind":"future_provider_error","deviceCode":"must-not-escape","authorizationBody":"also-private"}}}\n' "$id"
done
"#,
    )
    .expect("write fixture CLI");
    let mut permissions = fs::metadata(&cli).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cli, permissions).expect("make fixture executable");
    let config = Config::default()
        .with_cli_path(cli)
        .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start setup-only fixture");

    let error = sdk
        .begin_autohand_login()
        .await
        .expect_err("unknown setup RPC failure must fail closed");
    assert!(matches!(
        &error,
        Error::LoginFailed { problem }
            if problem.code == LoginProblemCode::ProtocolMismatch && !problem.retryable
    ));
    let rendered = format!("{error:?}\n{error}");
    assert!(!rendered.contains("must-not-escape"));
    assert!(!rendered.contains("also-private"));
    assert!(!rendered.contains("provider body must not escape"));
    sdk.stop().await.expect("stop fixture");
}

#[tokio::test]
async fn setup_authentication_errors_cannot_bypass_the_closed_problem_surface() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    fs::write(
        &cli,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32011,"message":"device code must-not-escape","data":{"kind":"authentication_required","providerId":"private-provider","deviceCode":"also-private"}}}\n' "$id"
done
"#,
    )
    .expect("write fixture CLI");
    let mut permissions = fs::metadata(&cli).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cli, permissions).expect("make fixture executable");
    let config = Config::default()
        .with_cli_path(cli)
        .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start setup-only fixture");

    let error = sdk
        .begin_autohand_login()
        .await
        .expect_err("setup authentication error must fail through the closed surface");
    assert!(matches!(
        &error,
        Error::LoginFailed { problem }
            if problem.code == LoginProblemCode::ProtocolMismatch && !problem.retryable
    ));
    let rendered = format!("{error:?}\n{error}");
    assert!(!rendered.contains("must-not-escape"));
    assert!(!rendered.contains("also-private"));
    assert!(!rendered.contains("private-provider"));
    sdk.stop().await.expect("stop fixture");
}

#[tokio::test]
async fn unbound_setup_errors_do_not_expose_private_authorization_data() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    fs::write(
        &cli,
        r#"#!/bin/sh
printf '%s\n' '{"jsonrpc":"2.0","id":null,"error":{"code":-32011,"message":"device code must-not-escape","data":{"kind":"authentication_required","providerId":"private-provider","deviceCode":"also-private"}}}'
while IFS= read -r ignored; do :; done
"#,
    )
    .expect("write fixture CLI");
    let mut permissions = fs::metadata(&cli).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cli, permissions).expect("make fixture executable");
    let config = Config::default()
        .with_cli_path(cli)
        .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start setup-only fixture");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let error = sdk
        .begin_autohand_login()
        .await
        .expect_err("an unbound setup error must fail closed");
    let rendered = format!("{error:?}\n{error}");
    assert!(matches!(error, Error::Protocol(_)));
    assert!(!rendered.contains("must-not-escape"));
    assert!(!rendered.contains("also-private"));
    assert!(!rendered.contains("private-provider"));
    sdk.stop().await.expect("stop fixture");
}
