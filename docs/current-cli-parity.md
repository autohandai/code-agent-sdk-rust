# Current CLI parity

Both `AutohandSdk` and `Agent` expose browser capability negotiation, thinking-token budgets, supported models, context usage, plugin reload, account information, feature settings, MCP server toggling/reconnection/replacement, and the existing permission, session, learning, registry, goal, auto-mode, and autoresearch controls.

`set_max_thinking_tokens(None)` sends the explicit JSON null accepted by the CLI. `toggle_mcp_server(name, false)` preserves false. `set_mcp_servers` and `apply_flag_settings` accept JSON objects; other shapes fail before an RPC request is written. Generic `request` remains available on `AutohandSdk` for future CLI methods.

## Per-run startup configuration

`Config::run_settings` accepts a `serde_json::Map<String, Value>` containing native provider sections, `customProviders`, `extensionProviders`, `features`, and `sessions.awareness`. Explicit `provider`, `model`, `api_key`, `base_url`, and typed `features` override only their matching leaves. Other supplied settings remain intact, including explicit false values.

```rust
use autohand_sdk::{Config, ProviderName};
use serde_json::json;

let config = Config {
    provider: Some(ProviderName::Anthropic),
    model: Some("your-model".into()),
    api_key: Some("your-key".into()),
    run_settings: json!({
        "features": {"slashGoal": false},
        "sessions": {"awareness": "passive"}
    }).as_object().cloned(),
    ..Config::default()
};
```

The SDK passes this overlay through `AUTOHAND_RUN_SETTINGS` and adds `--sdk-settings-version 1`. This requires a CLI supporting startup settings version 1. The CLI applies it before explicit profile and `--set` arguments, consumes the environment envelope before spawning tools, and restores saved configuration after the run. Credentials stay out of command-line arguments. `AUTOHAND_PROVIDER` selects the provider, and existing `AUTOHAND_AI_*` authentication variables remain supported. Supplying an API key selects API-key authentication for AutohandAI, OpenAI, xAI, and Azure. Bedrock uses `bedrock-api-key` authentication and requires a compatible `bedrock.apiMode` (`openai-chat` or `openai-responses`) in the saved configuration or run settings. Vertex AI maps `api_key` to its `authToken` and accepts a hostname or HTTPS origin in `base_url`; paths are rejected because the CLI constructs its project endpoint. Bedrock maps `base_url` to `endpoint`. Omitting keys preserves existing account authentication.

`ProviderName::Custom("custom:my.provider".into())` targets `customProviders["my.provider"]`. `ProviderName::Custom("extension:team.provider:model".into())` targets `extensionProviders["extension:team.provider:model"]`. Dots and colons in those IDs remain intact. Unsupported root settings, invalid section shapes, unsafe prototype path segments, and credentials without a provider fail before CLI discovery. Answer-only and setup-only profiles reject startup overlays.

## Forking sessions

Use `Config { fork_session: Some("session-id-or-path".into()), ..Default::default() }`. The legacy `fork: true` switch requires a `session_id` or `session_path` to supply the required CLI argument. A missing or blank source fails before process launch.

## Notification identity

`SdkEvent::notification_method()` returns the exact received JSON-RPC method. `notification_kind()` returns a typed `NotificationKind` for the current CLI methods, including peer and resource updates. `event_type` follows that canonical method even when the payload includes a different `type` field. For example, `autohand.pipe.output` has event type `pipe_output`, while its unchanged raw payload may have `type: "progress"`.

`raw` retains the original JSON payload, including unknown fields and malformed payloads. Typed family accessors return `Some(Err(...))` when a matching payload does not satisfy its schema. `decode_payload::<T>()` supports additional caller-defined payloads. Unknown notification methods retain their exact identity and payload. Autoresearch lifecycle decoding infers a missing phase from the method without adding fields to `raw`.

Use `SdkEvent { event_type, raw, .. }` when destructuring so the received `method` metadata remains available to other consumers.
