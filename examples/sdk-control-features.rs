mod common;

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    common::show_control_features().await
}
