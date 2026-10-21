#![cfg(unix)]

use autohand_sdk::{AutohandSdk, Config, ProviderName};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::PathBuf, time::Duration};
use tempfile::tempdir;

/// Opt-in integration proof against the actual CLI build. No prompt or model
/// request is issued; the saved config and all CLI state live in a temp folder.
#[tokio::test]
#[ignore = "requires AUTOHAND_SDK_TEST_CLI_PATH pointing to a CLI with SDK settings v1"]
async fn actual_cli_applies_startup_overlay_and_preserves_saved_provider_model() {
    let cli =
        PathBuf::from(std::env::var("AUTOHAND_SDK_TEST_CLI_PATH").expect("set actual CLI path"));
    let directory = tempdir().unwrap();
    let state_home = directory.path().join("state");
    fs::create_dir_all(&state_home).unwrap();
    let config_path = directory.path().join("config.json");
    fs::write(
        &config_path,
        json!({
            "provider":"ollama",
            "auth":{"token":"sdk-proof-fake-account-key"},
            "ollama":{"model":"saved-model","baseUrl":"http://127.0.0.1:9"},
            "telemetry":{"enabled":false},
            "features":{"slashGoal":true}
        })
        .to_string(),
    )
    .unwrap();
    let mut sdk = AutohandSdk::new(Config {
        cli_path: Some(cli),
        cwd: Some(directory.path().into()),
        bare: true,
        timeout: Duration::from_secs(30),
        provider: Some(ProviderName::Ollama),
        // Set only the raw startup overlay: --model cannot mask a failure.
        run_settings:
            json!({"ollama":{"model":"sdk-overlay-model"},"features":{"slashGoal":false}})
                .as_object()
                .cloned(),
        extra_args: vec![
            "--config".into(),
            config_path.to_string_lossy().into_owned(),
        ],
        env: BTreeMap::from([
            (
                "AUTOHAND_HOME".into(),
                state_home.to_string_lossy().into_owned(),
            ),
            ("AUTOHAND_AUTH_API_URL".into(), "http://127.0.0.1:9".into()),
            ("AUTOHAND_API_URL".into(), "http://127.0.0.1:9".into()),
        ]),
        ..Config::default()
    });
    sdk.start()
        .await
        .expect("start actual CLI with startup settings v1");
    let state = sdk.get_state().await.expect("read actual CLI state");
    assert_eq!(state["model"], "sdk-overlay-model");
    sdk.stop().await.expect("stop actual CLI");
    let saved: Value = serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(saved["ollama"]["model"], "saved-model");
    assert_eq!(saved["features"]["slashGoal"], true);
}
