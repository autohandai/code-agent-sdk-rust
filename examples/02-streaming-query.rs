mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level("02 Streaming Query", "Explain closures in one sentence").await
}
