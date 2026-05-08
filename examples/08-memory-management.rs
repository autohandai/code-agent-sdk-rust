mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "08 Memory Management",
        "Remember that this team prefers small Rust modules and focused tests. Then repeat it back.",
    )
    .await
}
