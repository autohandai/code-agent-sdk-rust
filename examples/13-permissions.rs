mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "13 Permissions",
        "Create a temporary file called autohand-rust-permission-demo.txt with one sentence.",
    )
    .await
}
