mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "10 Multi Tool Reasoning",
        "Inspect the repository layout, identify the SDK languages present, and summarize the examples.",
    )
    .await
}
