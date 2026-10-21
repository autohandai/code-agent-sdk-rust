#![cfg(unix)]

use autohand_sdk::{Agent, AutohandSdk, Config, FeatureFlagSettings, ProviderName};
use serde_json::{json, Value};
use std::{fs, os::unix::fs::PermissionsExt};
use tempfile::{tempdir, TempDir};

fn fixture(mut config: Config) -> (TempDir, AutohandSdk) {
    let directory = tempdir().unwrap();
    let cli = directory.path().join("autohand");
    fs::write(&cli, r#"#!/usr/bin/env python3
import json, os, sys
seen = []
for line in sys.stdin:
    request = json.loads(line)
    seen.append(request['method'])
    result = {'argv': sys.argv[1:], 'settings': json.loads(os.environ.get('AUTOHAND_RUN_SETTINGS', '{}')), 'provider': os.environ.get('AUTOHAND_PROVIDER'), 'seen': seen}
    print(json.dumps({'jsonrpc':'2.0', 'id':request['id'], 'result':result}), flush=True)
"#).unwrap();
    fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
    config.cli_path = Some(cli);
    (directory, AutohandSdk::new(config))
}

#[tokio::test]
async fn every_provider_receives_startup_settings_without_credentials_in_argv() {
    let providers = [
        ProviderName::AutohandAi,
        ProviderName::OpenRouter,
        ProviderName::Ollama,
        ProviderName::LlamaCpp,
        ProviderName::OpenAi,
        ProviderName::Anthropic,
        ProviderName::Mlx,
        ProviderName::LlmGateway,
        ProviderName::Azure,
        ProviderName::Zai,
        ProviderName::Sakana,
        ProviderName::Xai,
        ProviderName::Cerebras,
        ProviderName::DeepSeek,
        ProviderName::VertexAi,
        ProviderName::Nvidia,
        ProviderName::Bedrock,
        ProviderName::Custom("custom:my.provider".into()),
        ProviderName::Custom("extension:team.provider:model".into()),
    ];
    for provider in providers {
        let name = provider.as_str().to_owned();
        let (_directory, mut sdk) = fixture(Config {
            provider: Some(provider),
            model: Some("test-model".into()),
            api_key: Some("secret-test-key".into()),
            base_url: Some(if name == "vertexai" {
                "https://provider.test/".into()
            } else {
                "https://provider.test/api".into()
            }),
            features: Some(FeatureFlagSettings {
                slash_goal: Some(false),
                ..Default::default()
            }),
            run_settings: Some(
                json!({"features":{"usageV2":false},"sessions":{"awareness":"passive"}})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
            ..Config::default()
        });
        sdk.start().await.unwrap();
        let result = sdk.request("inspectStartup", json!({})).await.unwrap();
        assert_eq!(result["provider"], name);
        assert_eq!(
            result["settings"]["features"],
            json!({"slashGoal":false,"usageV2":false})
        );
        assert_eq!(result["settings"]["sessions"]["awareness"], "passive");
        let section = if let Some(id) = name.strip_prefix("custom:") {
            &result["settings"]["customProviders"][id]
        } else if name.starts_with("extension:") {
            &result["settings"]["extensionProviders"][&name]
        } else {
            &result["settings"][&name]
        };
        assert_eq!(section["model"], "test-model", "{name}");
        if name == "vertexai" {
            assert_eq!(section["authToken"], "secret-test-key");
            assert_eq!(section["endpoint"], "provider.test");
            assert!(section.get("apiKey").is_none());
            assert!(section.get("baseUrl").is_none());
        } else {
            assert_eq!(section["apiKey"], "secret-test-key", "{name}");
            let url_field = if name == "bedrock" {
                "endpoint"
            } else {
                "baseUrl"
            };
            assert_eq!(section[url_field], "https://provider.test/api", "{name}");
        }
        if name == "azure" {
            assert_eq!(section["authMethod"], "api-key");
        }
        if name == "bedrock" {
            assert_eq!(section["authMode"], "bedrock-api-key");
        }
        if matches!(name.as_str(), "autohandai" | "openai" | "xai") {
            assert_eq!(section["authMode"], "api-key");
        }
        if name == "autohandai" {
            assert_eq!(section["plan"], "cloud");
        }
        let args = result["argv"].as_array().unwrap();
        assert!(args
            .windows(2)
            .any(|pair| pair == [json!("--sdk-settings-version"), json!("1")]));
        assert!(!result["argv"].to_string().contains("secret-test-key"));
        assert!(!result["argv"]
            .to_string()
            .contains("https://provider.test/api"));
        assert_eq!(
            result["seen"],
            json!(["autohand.getState", "inspectStartup"])
        );
        sdk.stop().await.unwrap();
    }
}

#[tokio::test]
async fn explicit_provider_leaves_merge_without_changing_account_authentication() {
    let (_directory, mut sdk) = fixture(Config {
        provider: Some(ProviderName::AutohandAi),
        model: Some("explicit-model".into()),
        run_settings: Some(
            json!({"autohandai":{"authMode":"account","model":"old-model","orchestration":false}})
                .as_object()
                .unwrap()
                .clone(),
        ),
        ..Config::default()
    });
    sdk.start().await.unwrap();
    let result = sdk.request("inspectStartup", json!({})).await.unwrap();
    assert_eq!(
        result["settings"]["autohandai"],
        json!({"authMode":"account","model":"explicit-model","orchestration":false})
    );
    sdk.stop().await.unwrap();
}

#[tokio::test]
async fn fork_requires_and_passes_a_session_reference() {
    for config in [
        Config {
            fork_session: Some("source-id".into()),
            ..Config::default()
        },
        Config {
            fork: true,
            session_id: Some("source-id".into()),
            ..Config::default()
        },
        Config {
            fork: true,
            session_path: Some("source-id".into()),
            ..Config::default()
        },
    ] {
        let (_directory, mut sdk) = fixture(config);
        sdk.start().await.unwrap();
        let result = sdk.request("inspectStartup", json!({})).await.unwrap();
        assert!(result["argv"]
            .as_array()
            .unwrap()
            .windows(2)
            .any(|pair| pair == [json!("--fork"), json!("source-id")]));
        sdk.stop().await.unwrap();
    }
    for config in [
        Config {
            fork: true,
            ..Config::default()
        },
        Config {
            fork_session: Some(" ".into()),
            ..Config::default()
        },
    ] {
        let mut sdk = AutohandSdk::new(config);
        assert!(sdk
            .start()
            .await
            .unwrap_err()
            .to_string()
            .contains("fork requires"));
    }
}

#[tokio::test]
async fn invalid_run_settings_fail_before_cli_discovery() {
    for settings in [
        json!({"auth":{"token":"not-allowed"}}),
        json!({"provider":"openai"}),
        json!({"features":false}),
        json!({"sessions":{"persistSession":true}}),
        json!({"customProviders":{"sample":false}}),
        json!({"openai":{"nested":{"__proto__":{"polluted":true}}}}),
        json!({"features":{"a.constructor.b":true}}),
    ] {
        let mut sdk = AutohandSdk::new(Config {
            run_settings: settings.as_object().cloned(),
            ..Config::default()
        });
        assert!(matches!(
            sdk.start().await,
            Err(autohand_sdk::Error::InvalidInput(_))
        ));
    }
}

#[tokio::test]
async fn agent_runtime_controls_forward_exact_parameters() {
    let directory = tempdir().unwrap();
    let cli = directory.path().join("autohand");
    fs::write(&cli, include_str!("fixtures/current-cli-controls.py")).unwrap();
    fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
    let mut agent = Agent::create(Config {
        cli_path: Some(cli),
        ..Config::default()
    })
    .await
    .unwrap();
    let cases: [(Value, &str, Value); 10] = [
        (
            agent
                .set_browser_capabilities(2, "1.0.0", vec!["browser_navigate".into()])
                .await
                .unwrap(),
            "autohand.browserCapabilities.set",
            json!({"protocolVersion":2,"extensionVersion":"1.0.0","tools":["browser_navigate"]}),
        ),
        (
            agent.set_max_thinking_tokens(None).await.unwrap(),
            "autohand.maxThinkingTokensSet",
            json!({"maxThinkingTokens":null}),
        ),
        (
            agent.get_supported_models().await.unwrap(),
            "autohand.getSupportedModels",
            json!({}),
        ),
        (
            agent.get_context_usage().await.unwrap(),
            "autohand.getContextUsage",
            json!({}),
        ),
        (
            agent.reload_plugins().await.unwrap(),
            "autohand.reloadPlugins",
            json!({}),
        ),
        (
            agent.get_account_info().await.unwrap(),
            "autohand.getAccountInfo",
            json!({}),
        ),
        (
            agent.toggle_mcp_server("files", false).await.unwrap(),
            "autohand.mcp.toggleServer",
            json!({"serverName":"files","enabled":false}),
        ),
        (
            agent.reconnect_mcp_server("files").await.unwrap(),
            "autohand.mcp.reconnectServer",
            json!({"serverName":"files"}),
        ),
        (
            agent
                .set_mcp_servers(json!({"files":{"command":"server","args":[]}}))
                .await
                .unwrap(),
            "autohand.mcp.setServers",
            json!({"servers":{"files":{"command":"server","args":[]}}}),
        ),
        (
            agent
                .apply_flag_settings(json!({"features":{"slashGoal":false}}))
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
    agent.close().await.unwrap();
}
