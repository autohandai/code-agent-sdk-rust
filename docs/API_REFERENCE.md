# API Reference

## `Config`

Configuration used to start the Autohand CLI subprocess.

Common fields:

- `cwd`: working directory for the agent.
- `cli_path`: optional Autohand CLI binary path.
- `debug`: print CLI diagnostic output.
- `timeout`: JSON-RPC request timeout.
- `model`: model override passed to the CLI.
- `skills`: skills passed to the CLI.
- `append_system_prompt`: additional system instructions.
- `unrestricted`, `auto_mode`, `auto_skill`, `auto_commit`: execution mode flags.
- `context_compact`: enable or disable context compaction.
- `yolo`, `yolo_timeout_seconds`: unattended permission policy.

Helpers:

```rust
let config = Config::from_env()
    .with_cwd(".")
    .with_model("fantail2")
    .with_skill("rust")
    .with_instructions("Prefer small, typed Rust APIs.");
```

## `AutohandSdk`

Low-level JSON-RPC wrapper.

Important methods:

- `start()` / `stop()`
- `request(method, params)`
- `prompt(message, options)`
- `stream_prompt(message, options)`
- `interrupt()`
- `set_plan_mode(enabled)`
- `set_permission_mode(mode)`
- `set_model(model)`
- `get_state()`
- `get_messages()`
- `permission_response(request_id, decision)`

## `Agent`

High-level API for application code.

```rust
let mut agent = Agent::create(Config::from_env().with_cwd(".")).await?;
let mut run = agent.send("Review the public API.").await?;
let result = run.wait().await?;
agent.close().await?;
```

Methods:

- `Agent::create(config)`
- `Agent::from_sdk(sdk)`
- `send(prompt)`
- `run(prompt)`
- `run_json<T>(prompt, options)`
- `allow_permission(request_id)`
- `deny_permission(request_id)`
- `set_plan_mode(enabled)`
- `close()`

## `Run`

Represents a single agent run.

- `next()`: receive the next event.
- `wait()`: wait until the run finishes and collect text/events.
- `json<T>()`: parse final output as JSON.
- `abort()`: interrupt the current run.

## `SdkEvent`

Typed event envelope with raw JSON access.

Helpers:

- `text_delta()`
- `message_content()`
- `tool_name()`
- `request_id()`
- `description()`

Event types include:

- `agent_start`
- `turn_start`
- `message_update`
- `message_end`
- `tool_start`
- `tool_update`
- `tool_end`
- `permission_request`
- `error`

## Structured JSON

```rust
#[derive(serde::Deserialize)]
struct ReleaseRisk {
    summary: String,
}

let risk: ReleaseRisk = agent
    .run_json("Assess release readiness.", JsonRunOptions::default())
    .await?;
```
