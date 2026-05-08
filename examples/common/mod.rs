#![allow(dead_code)]

use autohand_sdk::{Agent, AutohandSdk, Config, JsonRunOptions, PromptOptions, Result, SdkEvent};
use serde::Deserialize;
use serde_json::json;

pub fn base_config() -> Config {
    Config::from_env().with_cwd(".")
}

pub async fn run_low_level(title: &str, prompt: &str) -> Result<()> {
    println!("=== {title} ===\n");
    let mut sdk = AutohandSdk::new(base_config());
    sdk.start().await?;
    let mut events = sdk
        .stream_prompt(prompt.to_string(), PromptOptions::default())
        .await?;
    while let Some(event) = events.recv().await {
        handle_event(&sdk, event?).await?;
    }
    let _ = sdk.get_state().await;
    sdk.stop().await?;
    Ok(())
}

pub async fn run_agent(title: &str, prompt: &str) -> Result<()> {
    println!("=== {title} ===\n");
    let mut agent = Agent::create(base_config()).await?;
    let mut run = agent.send(prompt).await?;
    while let Some(event) = run.next().await {
        handle_plain_event(event?).await;
    }
    let result = run.wait().await?;
    println!("\n\n=== Final Response ===\n{}", result.text);
    agent.close().await?;
    Ok(())
}

pub async fn run_json_example() -> Result<()> {
    #[derive(Debug, Deserialize)]
    struct ReleaseRisk {
        summary: String,
        risks: Vec<Risk>,
    }

    #[derive(Debug, Deserialize)]
    struct Risk {
        title: String,
        severity: String,
        mitigation: Option<String>,
    }

    let mut agent = Agent::create(base_config()).await?;
    let result: ReleaseRisk = agent
        .run_json(
            "Assess this SDK repository for publish readiness. Do not execute commands.",
            JsonRunOptions {
                schema_name: Some("ReleaseRisk".to_string()),
                schema: Some(json!({
                    "summary": "string",
                    "risks": [{
                        "title": "string",
                        "severity": "low | medium | high",
                        "mitigation": "string"
                    }]
                })),
                output_instructions: Some(
                    "If you cannot inspect the repository, still return a JSON object.".to_string(),
                ),
            },
        )
        .await?;
    println!("{}", result.summary);
    for risk in result.risks {
        println!(
            "- {}: {}{}",
            risk.severity,
            risk.title,
            risk.mitigation
                .as_deref()
                .map(|m| format!(" ({m})"))
                .unwrap_or_default()
        );
    }
    agent.close().await?;
    Ok(())
}

pub async fn show_control_features() -> Result<()> {
    let sdk = AutohandSdk::new(base_config());
    let methods = [
        "request",
        "prompt",
        "stream_prompt",
        "interrupt",
        "set_plan_mode",
        "set_permission_mode",
        "set_model",
        "get_state",
        "get_messages",
        "permission_response",
    ];
    for method in methods {
        println!("✓ SDK has method: {method}");
    }
    drop(sdk);
    Ok(())
}

pub async fn handle_event(sdk: &AutohandSdk, event: SdkEvent) -> Result<()> {
    if event.event_type == "permission_request" {
        println!(
            "\n[permission] {}: {}",
            event.tool_name().unwrap_or("unknown"),
            event.description().unwrap_or("")
        );
        if let Some(request_id) = event.request_id() {
            let _ = sdk.permission_response(request_id, "allow_once").await?;
        }
        return Ok(());
    }
    handle_plain_event(event).await;
    Ok(())
}

pub async fn handle_plain_event(event: SdkEvent) {
    match event.event_type.as_str() {
        "agent_start" => println!("[agent started]"),
        "turn_start" => println!("[turn started]"),
        "message_update" => {
            if let Some(delta) = event.text_delta() {
                print!("{delta}");
            }
        }
        "message_end" => println!("\n[message completed]"),
        "tool_start" => println!("\n[tool] {}", event.tool_name().unwrap_or("unknown")),
        "tool_update" => {
            if let Some(output) = event.raw.get("output").and_then(|v| v.as_str()) {
                print!("{output}");
            }
        }
        "tool_end" => println!(
            "\n[tool completed] {}",
            event.tool_name().unwrap_or("unknown")
        ),
        "permission_request" => println!(
            "\n[permission] {}: {}",
            event.tool_name().unwrap_or("unknown"),
            event.description().unwrap_or("")
        ),
        "error" => eprintln!("\n[error] {}", event.raw),
        _ => {}
    }
}
