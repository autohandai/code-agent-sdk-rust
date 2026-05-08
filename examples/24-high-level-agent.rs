mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_agent("24 High-Level Agent", "Summarize the API surface").await
}
