mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "Permission Handling",
        "Create a new file called autohand-rust-sdk-demo.txt with some content.",
    )
    .await
}
