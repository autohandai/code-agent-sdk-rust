# Getting Started

The Autohand Code Agent SDK for Rust spawns the Autohand CLI in JSON-RPC mode and exposes async Rust APIs for prompts, streaming events, permissions, and structured output.

## Prerequisites

1. Install and authenticate the Autohand CLI.
2. Configure a provider in `~/.autohand/config.json`.
3. Install Rust and Cargo.

Set a custom CLI path when developing locally:

```bash
export AUTOHAND_CLI_PATH=/path/to/autohand
```

## Installation

Until the crate is published, use the GitHub repo directly:

```toml
[dependencies]
autohand-sdk = { git = "https://github.com/autohandai/code-agent-sdk-rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Your First Agent

```rust
use autohand_sdk::{Agent, Config, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut agent = Agent::create(Config::from_env().with_cwd(".")).await?;
    let result = agent.run("Summarize this repository.").await?;
    println!("{}", result.text);
    agent.close().await?;
    Ok(())
}
```

## Streaming

```rust
use autohand_sdk::{AutohandSdk, Config, PromptOptions, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut sdk = AutohandSdk::new(Config::from_env().with_cwd("."));
    sdk.start().await?;

    let mut events = sdk
        .stream_prompt("Explain the SDK in one paragraph.", PromptOptions::default())
        .await?;

    while let Some(event) = events.recv().await {
        let event = event?;
        if let Some(delta) = event.text_delta() {
            print!("{delta}");
        }
    }

    sdk.stop().await?;
    Ok(())
}
```

## Next Steps

- Read [Configuration](./configuration.md).
- Try [Event Streaming](./event-streaming.md).
- Learn [Permissions](./permissions.md).
- Use [SDLC Workflows](./sdlc-workflows.md) for production changes.
