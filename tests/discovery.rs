#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt};

use autohand_sdk::{
    AutohandSdk, Config, GetSkillsRegistryParams, InstallSkillParams, McpListToolsParams,
    McpTransport, SkillInstallScope,
};
use serde_json::Value;
use tempfile::tempdir;

#[tokio::test]
async fn routes_all_five_discovery_methods_with_typed_results() {
    let dir = tempdir().unwrap();
    let cli = dir.path().join("fake-autohand");
    let log = dir.path().join("requests.jsonl");
    fs::write(
        &cli,
        format!(
            r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "{}"
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.getSkillsRegistry*) result='{{"success":true,"skills":[{{"id":"skill-1","name":"review","description":"Review code","category":"quality"}}],"categories":[{{"name":"quality","count":1}}]}}' ;;
    *autohand.installSkill*) result='{{"success":true,"skillName":"review","path":"/skills/review"}}' ;;
    *autohand.mcp.listServers*) result='{{"servers":[{{"name":"github","status":"connected","toolCount":2}}]}}' ;;
    *autohand.mcp.listTools*) result='{{"tools":[{{"name":"search","description":"Search issues","serverName":"github"}}]}}' ;;
    *autohand.mcp.getServerConfigs*) result='{{"configs":[{{"name":"github","transport":"stdio","command":"mcp-github","args":[],"env":{{}},"headers":{{}}}}]}}' ;;
    *) result='{{"ready":true}}' ;;
  esac
  printf '{{"jsonrpc":"2.0","id":%s,"result":%s}}\n' "$id" "$result"
done
"#,
            log.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&cli).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cli, permissions).unwrap();

    let mut sdk = AutohandSdk::new(Config::default().with_cli_path(&cli));
    sdk.start().await.unwrap();
    let registry = sdk
        .get_skills_registry(GetSkillsRegistryParams {
            force_refresh: Some(true),
        })
        .await
        .unwrap();
    assert_eq!(registry.skills[0].id, "skill-1");

    let installed = sdk
        .install_skill(InstallSkillParams {
            skill_name: "review".into(),
            scope: SkillInstallScope::Project,
            force: None,
        })
        .await
        .unwrap();
    assert_eq!(installed.path.as_deref(), Some("/skills/review"));
    assert_eq!(
        sdk.list_mcp_servers().await.unwrap().servers[0].tool_count,
        2
    );
    assert_eq!(
        sdk.list_mcp_tools(McpListToolsParams {
            server_name: Some("github".into()),
        })
        .await
        .unwrap()
        .tools[0]
            .server_name,
        "github"
    );
    assert_eq!(
        sdk.get_mcp_server_configs().await.unwrap().configs[0].transport,
        McpTransport::Stdio
    );
    sdk.stop().await.unwrap();

    let requests: Vec<Value> = fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let routed: Vec<&Value> = requests
        .iter()
        .filter(|request| request["method"] != "autohand.getState")
        .collect();
    assert_eq!(
        routed
            .iter()
            .map(|request| request["method"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "autohand.getSkillsRegistry",
            "autohand.installSkill",
            "autohand.mcp.listServers",
            "autohand.mcp.listTools",
            "autohand.mcp.getServerConfigs",
        ]
    );
    assert_eq!(routed[0]["params"]["forceRefresh"], true);
    assert_eq!(routed[1]["params"]["skillName"], "review");
    assert_eq!(routed[1]["params"]["scope"], "project");
    assert_eq!(routed[3]["params"]["serverName"], "github");
}

#[tokio::test]
async fn install_skill_rejects_an_empty_name_before_transport_access() {
    let sdk = AutohandSdk::new(Config::default());
    let error = sdk
        .install_skill(InstallSkillParams {
            skill_name: "  ".into(),
            scope: SkillInstallScope::User,
            force: None,
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("skill_name is required"));
}
