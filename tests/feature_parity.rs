#![cfg(unix)]

use autohand_sdk::{
    format_slash_command, AutohandSdk, Config, FeatureFlagSettings, GoalCreateParams,
    GoalUpdateParams, ProviderName, SdkEvent, TokenUsageStatus,
};
use std::{fs, os::unix::fs::PermissionsExt};
use tempfile::tempdir;

#[test]
fn slash_goal_params_provider_env_and_turn_usage_are_typed() {
    assert_eq!(
        format_slash_command(" /deep-research ", &[" find ", "", "evidence"]).unwrap(),
        "/deep-research find evidence"
    );
    assert!(format_slash_command("deep-research", &[] as &[&str]).is_err());
    let params = GoalUpdateParams {
        token_budget: Some(None),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(params).unwrap(),
        serde_json::json!({"token_budget":null})
    );
    let mut config = Config {
        provider: Some(ProviderName::AutohandAi),
        api_key: Some("key".into()),
        base_url: Some("https://api".into()),
        ..Default::default()
    };
    config.env.insert("AUTOHAND_AI_PLAN".into(), "max".into());
    // Explicit environment entries are the final authority.
    assert_eq!(
        config.env.get("AUTOHAND_AI_PLAN").map(String::as_str),
        Some("max")
    );
    let event = SdkEvent::new(
        "turn_end",
        serde_json::json!({"tokensUsed":9,"tokensUsageStatus":"actual","durationMs":4,"contextPercent":2.5}),
    );
    let usage = event.turn_end_usage().unwrap().unwrap();
    assert_eq!(usage.tokens_used, Some(9));
    assert_eq!(usage.tokens_usage_status, Some(TokenUsageStatus::Actual));
}

#[tokio::test]
async fn routes_feature_discovery_and_all_persistent_goal_rpcs() {
    let dir = tempdir().unwrap();
    let cli = dir.path().join("fake-autohand");
    fs::write(&cli,r#"#!/bin/sh
while IFS= read -r line; do
 id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
 case "$line" in
  *autohand.applyFlagSettings*) result='{"ok":true}' ;;
  *autohand.getSupportedCommands*) result='{"commands":["goal","/deep-research"]}' ;;
  *autohand.goal.get*) result='{"version":1,"goal":null,"queue":[],"completed":[],"updatedAt":"now"}' ;;
  *autohand.goal.listTemplates*) result='[]' ;;
  *autohand.goal.create*|*autohand.goal.update*|*autohand.goal.queue*|*autohand.goal.startQueued*|*autohand.goal.clear*) result='{"ok":true,"goal":null,"queue":[]}' ;;
  *) result='{"unexpected":true}' ;;
 esac
 printf '{"jsonrpc":"2.0","id":%s,"result":%s}\n' "$id" "$result"
done
"#).unwrap();
    let mut permissions = fs::metadata(&cli).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cli, permissions).unwrap();
    let mut config = Config::default().with_cli_path(&cli);
    config.features = Some(FeatureFlagSettings {
        slash_goal: Some(true),
        ..Default::default()
    });
    let mut sdk = AutohandSdk::new(config);
    sdk.start().await.unwrap();
    assert_eq!(
        sdk.supported_commands().await.unwrap(),
        vec!["/goal", "/deep-research"]
    );
    assert!(sdk.supports_command("goal").await.unwrap());
    sdk.get_goal().await.unwrap();
    sdk.create_goal(GoalCreateParams::new("ship"))
        .await
        .unwrap();
    sdk.update_goal(GoalUpdateParams::default()).await.unwrap();
    sdk.queue_goal(GoalCreateParams::new("next")).await.unwrap();
    sdk.start_queued_goal().await.unwrap();
    sdk.list_goal_templates().await.unwrap();
    sdk.clear_goal().await.unwrap();
    sdk.stop().await.unwrap();
}
