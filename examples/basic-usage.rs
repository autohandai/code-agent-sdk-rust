mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level("Basic Usage", "Hello, Autohand!").await
}
