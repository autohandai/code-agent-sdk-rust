#![cfg(unix)]
use autohand_sdk::{AutohandSdk, Config};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt};
use tempfile::tempdir;

#[tokio::test]
async fn current_runtime_controls_use_exact_wire_parameters() {
    let directory = tempdir().unwrap();
    let cli = directory.path().join("autohand");
    fs::write(&cli, include_str!("fixtures/current-cli-controls.py")).unwrap();
    fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
    let mut sdk = AutohandSdk::new(Config {
        cli_path: Some(cli),
        ..Config::default()
    });
    sdk.start().await.unwrap();
    let cases = [
        (
            sdk.set_browser_capabilities(2, "1.0.0", vec!["browser_navigate".into()])
                .await
                .unwrap(),
            "autohand.browserCapabilities.set",
            json!({"protocolVersion":2,"extensionVersion":"1.0.0","tools":["browser_navigate"]}),
        ),
        (
            sdk.set_max_thinking_tokens(None).await.unwrap(),
            "autohand.maxThinkingTokensSet",
            json!({"maxThinkingTokens":null}),
        ),
        (
            sdk.get_supported_models().await.unwrap(),
            "autohand.getSupportedModels",
            json!({}),
        ),
        (
            sdk.get_context_usage().await.unwrap(),
            "autohand.getContextUsage",
            json!({}),
        ),
        (
            sdk.reload_plugins().await.unwrap(),
            "autohand.reloadPlugins",
            json!({}),
        ),
        (
            sdk.get_account_info().await.unwrap(),
            "autohand.getAccountInfo",
            json!({}),
        ),
        (
            sdk.toggle_mcp_server("files", false).await.unwrap(),
            "autohand.mcp.toggleServer",
            json!({"serverName":"files","enabled":false}),
        ),
        (
            sdk.reconnect_mcp_server("files").await.unwrap(),
            "autohand.mcp.reconnectServer",
            json!({"serverName":"files"}),
        ),
        (
            sdk.set_mcp_servers(json!({"files":{"command":"server","args":[]}}))
                .await
                .unwrap(),
            "autohand.mcp.setServers",
            json!({"servers":{"files":{"command":"server","args":[]}}}),
        ),
        (
            sdk.apply_flag_settings(json!({"features":{"slashGoal":false}}))
                .await
                .unwrap(),
            "autohand.applyFlagSettings",
            json!({"settings":{"features":{"slashGoal":false}}}),
        ),
    ];
    for (result, method, params) in cases {
        assert_eq!(result["method"], method);
        assert_eq!(result["params"], params);
    }
    sdk.stop().await.unwrap();
}
