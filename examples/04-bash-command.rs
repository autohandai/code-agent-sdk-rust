mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "04 Bash Command",
        "What is the current directory listing and total file count?",
    )
    .await
}
