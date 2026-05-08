# Configuration

The Rust SDK keeps configuration close to the Autohand CLI contract. Most fields become CLI flags when the subprocess starts.

## Basic Configuration

```rust
let config = Config::from_env()
    .with_cwd(".")
    .with_model("fantail2")
    .with_skill("rust")
    .with_instructions("Prefer safe, idiomatic Rust.");
```

`Config::from_env()` reads `AUTOHAND_CLI_PATH` when present.

## Provider Credentials

Provider credentials are owned by the Autohand CLI, not the SDK. Configure them in `~/.autohand/config.json` or through environment variables supported by the CLI.

```json
{
  "provider": "openrouter",
  "openrouter": {
    "apiKey": "sk-or-...",
    "model": "openrouter/auto"
  }
}
```

## Runtime Options

Common options:

- `model`: model override.
- `temperature`: sampling temperature.
- `max_iterations`: loop limit.
- `max_runtime_minutes`: wall-clock limit.
- `max_cost`: cost budget.
- `context_compact`: context compaction.
- `additional_directories`: extra workspace roots.
- `skills`: skills available to the agent.
- `env`: environment variables for the CLI subprocess.

## System Prompts

Use `with_instructions()` or `append_system_prompt` for normal integrations. Replacing `system_prompt` means your host owns the full agent contract.

```rust
let config = Config::from_env()
    .with_instructions("Return concise findings with file references.");
```

## Permissions

Use `unrestricted` only for trusted automation. For most applications, keep the default interactive behavior and respond to `permission_request` events.

```rust
sdk.set_permission_mode("interactive").await?;
```

## Plan Mode

Plan mode is a runtime control:

```rust
sdk.set_plan_mode(true).await?;
```

See [Plan Mode](./plan-mode.md).
