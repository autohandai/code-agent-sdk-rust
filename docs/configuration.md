# Configuration

## Process provider selection

Set `Config.provider` to the canonical provider name (for example, `autohandai`).
The SDK forwards it as `AUTOHAND_PROVIDER` after other environment overrides.
With [CLI provider startup support](https://github.com/autohandai/code-cli/commit/240f071013316ebed4fcffb5af68f98cf2f8b2ff), this selects the provider ahead of global and workspace settings.
When no provider is configured or inferred by the SDK, normal CLI environment
and saved configuration selection apply. Older CLIs may ignore the override;
use a CLI containing the linked change.

Autohand AI inference credentials use `AUTOHAND_AI_API_KEY`,
`AUTOHAND_AI_BASE_URL`, and `AUTOHAND_AI_PLAN`. Account authentication is
separate, and configured feature gates still apply. The CLI retains saved
provider settings and credentials when other settings are saved during the run.
Restricted Blueprint and login profiles continue to reject provider overrides.

`Config` mirrors the current CLI RPC launch surface, including bare mode,
idle-logout control, session persistence/resume/continue/fork, AGENTS.md
controls, token thresholds, skill sources, display language, prompt files, and
MCP/agent/plugin paths. `features` is applied through
`autohand.applyFlagSettings` immediately after startup.

For AutohandAI, use `ProviderName::AutohandAi` with `api_key`, `base_url`, and
`autohand_ai_plan`, or load `AUTOHAND_AI_API_KEY`, `AUTOHAND_AI_BASE_URL`, and
`AUTOHAND_AI_PLAN` through `Config::from_env()`. Explicit `Config::env` values
take precedence.

The Rust SDK keeps configuration close to the Autohand CLI contract. Most fields become CLI flags when the subprocess starts.

## Mutually Exclusive Runtime Profiles

Normal callers use the default interactive profile. Blueprint uses one of two
closed profiles:

```rust
let answer = Config::default()
    .with_cli_path("/reviewed/path/to/autohand")
    .with_answer_only_profile(AnswerOnlyProfile::blueprint());

let setup = Config::default()
    .with_cli_path("/reviewed/path/to/autohand")
    .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization());
```

Answer-only always launches with `--answer-only --restricted
--client-context blueprint`, clears the inherited environment, and requires
deny-all child egress. Setup-only uses a distinct `--setup-only` profile and
admits only the versioned Autohand device-authorization traffic class. Neither
profile can call the interactive prompt/control RPC surface.

`extra_args` and interactive/workspace/provider overrides are rejected for
both restricted profiles. The SDK restores only minimal platform process
variables plus names explicitly selected by `allow_environment`; direct
`Config::env` injection is rejected for these profiles. Blueprint production
configuration should keep the explicit allowlist empty unless a reviewed
runtime dependency requires a named variable. `HOME`/`USERPROFILE` is
preserved so the CLI can use its normal credential owner without the SDK
reading or copying a credential.

Behavior-changing Autohand endpoint variables, proxy variables, and process
injection variables such as `NODE_OPTIONS`, `LD_PRELOAD`, and
`DYLD_INSERT_LIBRARIES` cannot be restored by the restricted allowlist.
TLS trust overrides such as `NODE_EXTRA_CA_CERTS` are also forbidden.

Restricted output and capture limits may be lowered for a deployment, but
cannot be raised above the version-1 contract defaults.

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
- `max_events`, `max_event_bytes`: event collection limits.
- `max_stdout_bytes`, `max_stderr_bytes`: subprocess capture limits.

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
