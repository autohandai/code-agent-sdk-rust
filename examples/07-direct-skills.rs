use autohand_sdk::{AutohandSdk, Config, PromptOptions};

mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    let config = Config::from_env()
        .with_cwd(".")
        .with_model("fantail2")
        .with_skill("rust")
        .with_skill("testing");
    let mut sdk = AutohandSdk::new(config);
    sdk.start().await?;
    let mut events = sdk
        .stream_prompt(
            "Review this codebase and suggest improvements.",
            PromptOptions::default(),
        )
        .await?;
    while let Some(event) = events.recv().await {
        common::handle_event(&sdk, event?).await?;
    }
    sdk.stop().await
}
