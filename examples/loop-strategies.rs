mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "Loop Strategies",
        "List all Rust files in the current directory and summarize the codebase.",
    )
    .await
}
