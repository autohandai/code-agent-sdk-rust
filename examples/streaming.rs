mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level("Streaming", "Analyze the current directory structure").await
}
