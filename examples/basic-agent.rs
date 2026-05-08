mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "Basic Agent",
        "Hello, what can you help me with today? Mention session and context features.",
    )
    .await
}
