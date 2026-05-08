mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::run_low_level(
        "21 SDLC Gated Implementation",
        "Implement the approved plan, but ask for permission before file writes or shell commands.",
    )
    .await
}
