mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "05 File Editor",
        "Read README.md and fix any obvious typos in comments.",
    )
    .await
}
