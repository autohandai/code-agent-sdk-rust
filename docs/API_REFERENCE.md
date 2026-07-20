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

`autohand_sdk::initialize()` is an optional, idempotent eager-initialization
hook. Normal SDK construction works without calling it explicitly; the startup
benchmark uses it to isolate public crate initialization in a fresh process.

Important methods:

- `start()` / `stop()`
- `request(method, params)`
- `prompt(message, options)`
- `stream_prompt(message, options)`
- `interrupt()`
- `reset()`
- `set_plan_mode(enabled)`
- `reset()`
- `set_permission_mode(mode)`
- `set_model(model)`
- `get_state()`
- `get_messages()`
- `permission_response(request_id, decision)`
- `start_autoresearch(params)` / `get_autoresearch_status()` / `stop_autoresearch()`
- `get_autoresearch_history()` / `replay_autoresearch(params)`
- `rescore_autoresearch(params)` / `compare_autoresearch(params)`
- `get_autoresearch_pareto()` / `pin_autoresearch(params)` / `prune_autoresearch(params)`
- `stream_command(command, args)` / `supported_commands()` / `supports_command(command)`
- `get_goal()` / `create_goal(params)` / `update_goal(params)` / `queue_goal(params)`
- `start_queued_goal()` / `list_goal_templates()` / `clear_goal()`
- `get_skills_registry(params)` / `install_skill(params)`
- `list_mcp_servers()` / `list_mcp_tools(params)` / `get_mcp_server_configs()`

### Skill Registry And MCP Discovery

```rust
use autohand_sdk::{
    GetSkillsRegistryParams, InstallSkillParams, McpListToolsParams,
    SkillInstallScope,
};

let registry = sdk
    .get_skills_registry(GetSkillsRegistryParams {
        force_refresh: Some(true),
    })
    .await?;
let installed = sdk
    .install_skill(InstallSkillParams {
        skill_name: "code-review".into(),
        scope: SkillInstallScope::Project,
        force: None,
    })
    .await?;
let servers = sdk.list_mcp_servers().await?;
let tools = sdk
    .list_mcp_tools(McpListToolsParams {
        server_name: Some("github".into()),
    })
    .await?;
let configs = sdk.get_mcp_server_configs().await?;
```

The MCP configuration transport is the closed `McpTransport::{Stdio, Sse,
Http}` enum. Optional registry metadata and server configuration fields remain
optional during deserialization.

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
- `autoresearch(objective)`
- `command(command, args)` / `deep_research(objective)`
- All typed persistent-goal methods exposed by `AutohandSdk`
- All typed autoresearch lifecycle and ledger methods exposed by `AutohandSdk`
- All typed skill registry and MCP discovery methods exposed by `AutohandSdk`

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
- `autoresearch` (decode with `SdkEvent::autoresearch()`)

## Replayable Autoresearch

`AutoresearchStartParams` configures the objective, primary and secondary
metrics, constraints, adaptive sampling, file scope, subagents, and retention.
Both `Agent` and `AutohandSdk` expose typed methods for start/status/stop,
history, replay, rescore, compare, Pareto, pin, and prune.

`AutoresearchRescoreParams::attempt(id)` and
`AutoresearchRescoreParams::all()` make mutually exclusive selections explicit.
`SdkEvent::autoresearch()` decodes lifecycle and ledger-operation notifications.
`AutoresearchHookEvent` provides the canonical CLI hook names.

See [Replayable Autoresearch](./autoresearch.md) for complete examples and
retention safety guidance.

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
