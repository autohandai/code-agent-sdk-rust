mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "22 SDLC Release Readiness",
        "Review this SDK for release readiness. Focus on docs, examples, and tests.",
    )
    .await
}
