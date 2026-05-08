mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "01 Hello Agent",
        "Tell me a good joke about code AI agents!",
    )
    .await
}
