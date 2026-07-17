# Autohand Code Agent SDK for Rust

Rust SDK for building applications that control Autohand code agents through the Autohand CLI JSON-RPC mode.

**Documentation:** https://autohand.ai/docs/agent-sdk/

**Beta:** this SDK is actively evolving while the Agent SDK APIs stabilize. Pin versions in production and review release notes before upgrading.

## What It Does

The Rust SDK wraps the existing Autohand CLI process and gives Rust applications an async, typed API for agent runs:

```text
Rust app -> autohand-sdk -> Autohand CLI subprocess -> provider -> model
```

Use it when you want to embed Autohand inside a Rust service, developer tool, CLI, desktop app, or test harness while keeping the same CLI behavior users already trust.

## Features

- Tokio-based subprocess transport over JSON-RPC 2.0
- `Agent` and `Run` for high-level application workflows
- `AutohandSdk` for direct low-level RPC access
- Typed streaming events for messages, tools, permissions, and errors
- Permission response helpers for host-controlled approval flows
- Structured JSON helpers for typed agent output
- Example parity with the TypeScript SDK examples
- Typed replayable autoresearch lifecycle, ledger operations, events, and hooks
- Validated generic slash commands plus deep-research/autoresearch helpers
- Seven typed persistent-goal RPCs and live command discovery
- Current session, AGENTS.md, token, skill-source, prompt-file, MCP, agent, and plugin flags
- Startup feature settings, typed turn usage, and AutohandAI environment support

## Requirements

- Rust 1.80 or later
- Tokio runtime
- Autohand CLI installed and authenticated
- A configured provider in `~/.autohand/config.json`, or environment variables accepted by the CLI

Set `AUTOHAND_CLI_PATH` when you want to force a local CLI binary:

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

The crate name is planned as `autohand-sdk`.

## Quick Start

```rust
use autohand_sdk::{Agent, Config, Result, SdkEvent};

#[tokio::main]
async fn main() -> Result<()> {
    let mut agent = Agent::create(
        Config::from_env()
            .with_cwd(".")
            .with_instructions("Review code with senior Rust judgement."),
    )
    .await?;

    let mut run = agent.send("Review this repository for release readiness.").await?;

    while let Some(event) = run.next().await {
        match event? {
            SdkEvent { event_type, raw } if event_type == "message_update" => {
                if let Some(delta) = raw.get("delta").and_then(|value| value.as_str()) {
                    print!("{delta}");
                }
            }
            SdkEvent { event_type, raw } if event_type == "permission_request" => {
                eprintln!("permission requested: {raw}");
            }
            _ => {}
        }
    }

    let result = run.wait().await?;
    println!("\nRun {} finished with {}", result.id, result.status);

    agent.close().await?;
    Ok(())
}
```

## Low-Level Control

Use `AutohandSdk` when your host needs direct access to the JSON-RPC control surface:

```rust
use autohand_sdk::{AutohandSdk, Config, PromptOptions, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut sdk = AutohandSdk::new(Config::from_env().with_cwd("."));
    sdk.start().await?;

    sdk.set_plan_mode(true).await?;

    let mut events = sdk
        .stream_prompt(
            "Create a discovery plan for this SDK change.",
            PromptOptions::default(),
        )
        .await?;

    while let Some(event) = events.recv().await {
        println!("{:?}", event?);
    }

    sdk.stop().await?;
    Ok(())
}
```

## Examples

The `examples/` directory mirrors the TypeScript SDK example inventory:

- `01-hello-agent.rs`
- `02-streaming-query.rs`
- `03-code-reviewer.rs`
- `04-bash-command.rs`
- `05-file-editor.rs`
- `06-prompt-skills.rs`
- `07-direct-skills.rs`
- `08-memory-management.rs`
- `10-multi-tool-reasoning.rs`
- `13-permissions.rs`
- `20-sdlc-discovery-plan.rs`
- `21-sdlc-gated-implementation.rs`
- `22-sdlc-release-readiness.rs`
- `23-system-prompts.rs`
- `24-high-level-agent.rs`
- `25-structured-json.rs`
- `27-autoresearch-ledger.rs`
- `basic-agent.rs`
- `basic-usage.rs`
- `loop-strategies.rs`
- `permission-handling.rs`
- `sdk-control-features.rs`
- `streaming.rs`

Run an example with:

```bash
cargo run --example 01-hello-agent
```

Live examples require an authenticated Autohand CLI and may ask for tool permissions depending on your CLI configuration.

## Documentation

- [Getting Started](./docs/getting-started.md)
- [API Reference](./docs/API_REFERENCE.md)
- [Configuration](./docs/configuration.md)
- [Event Streaming](./docs/event-streaming.md)
- [Permissions](./docs/permissions.md)
- [Plan Mode](./docs/plan-mode.md)
- [SDLC Workflows](./docs/sdlc-workflows.md)
- [Error Handling](./docs/error-handling.md)
- [Examples](./docs/examples.md)
- [Replayable Autoresearch](./docs/autoresearch.md)
- [Contributing](./CONTRIBUTING.md)
- [Security](./SECURITY.md)

## Development

```bash
cargo fmt --check
cargo test
cargo check --examples
```

The transport tests use a deterministic fake CLI, so the unit suite does not require model credentials.

## Other SDKs

- [TypeScript](https://github.com/autohandai/code-agent-sdk-typescript)
- [Python](https://github.com/autohandai/code-agent-sdk-python)
- [Go](https://github.com/autohandai/code-agent-sdk-go)
- [Java](https://github.com/autohandai/code-agent-sdk-java)
- [Swift](https://github.com/autohandai/code-agent-sdk-swift)
- [C++](https://github.com/autohandai/code-agent-sdk-cpp)
- [C#](https://github.com/autohandai/code-agent-sdk-csharp)

## Support

- SDK docs: https://autohand.ai/docs/agent-sdk/
- Issues: https://github.com/autohandai/code-agent-sdk-rust/issues
- Security reports: security@autohand.ai
