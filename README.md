# Code Agent SDK for Rust

Rust SDK for controlling Autohand code agents through the CLI JSON-RPC mode.

**Beta:** this SDK is actively evolving while the Agent SDK APIs stabilize. Pin versions in production and review release notes before upgrading.

## Quick Start

```rust
use autohand_sdk::{Agent, Config, Result, SdkEvent};

#[tokio::main]
async fn main() -> Result<()> {
    let mut agent = Agent::create(
        Config::from_env().with_instructions("Review code with senior Rust judgement.")
    ).await?;

    let mut run = agent.send("Summarize this repository").await?;

    while let Some(event) = run.next().await {
        match event? {
            SdkEvent { event_type, raw } if event_type == "message_update" => {
                if let Some(delta) = raw.get("delta").and_then(|v| v.as_str()) {
                    print!("{delta}");
                }
            }
            _ => {}
        }
    }

    agent.close().await?;
    Ok(())
}
```

## Development

```bash
cargo fmt --check
cargo test
cargo check --examples
```

The `examples/` directory mirrors the TypeScript SDK examples, covering streaming, permissions, structured JSON, plan mode, high-level agents, and SDK control methods.

Live examples require an authenticated Autohand CLI. Set `AUTOHAND_CLI_PATH` to force a local development binary.
