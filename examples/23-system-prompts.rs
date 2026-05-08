use autohand_sdk::{Agent, Config};

mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    let mut agent = Agent::create(
        Config::from_env()
            .with_cwd(".")
            .with_instructions("You are a concise Rust SDK reviewer. Prefer actionable notes."),
    )
    .await?;
    let result = agent
        .run("Summarize the public API in three bullets.")
        .await?;
    println!("{}", result.text);
    agent.close().await
}
