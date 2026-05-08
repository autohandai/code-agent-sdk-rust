use autohand_sdk::{AutohandSdk, Config, PromptOptions};

mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    let mut sdk = AutohandSdk::new(Config::from_env().with_cwd("."));
    sdk.start().await?;
    let _ = sdk.set_plan_mode(true).await;
    let mut events = sdk
        .stream_prompt(
            "Inspect this package and produce an implementation plan. Do not edit files.",
            PromptOptions::default(),
        )
        .await?;
    while let Some(event) = events.recv().await {
        common::handle_event(&sdk, event?).await?;
    }
    sdk.stop().await
}
