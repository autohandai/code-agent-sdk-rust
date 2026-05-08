mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_json_example().await
}
