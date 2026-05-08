mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "03 Code Reviewer",
        "What Rust and TypeScript files are in the current directory? Read each one and report any issues.",
    )
    .await
}
