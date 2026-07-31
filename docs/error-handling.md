# Error Handling

SDK errors fall into four categories.

## Transport Errors

The CLI subprocess could not start, disconnected, or returned invalid JSON.

Common causes:

- `AUTOHAND_CLI_PATH` points to a missing binary.
- The CLI is not authenticated.
- The provider config is invalid.

## Request Timeouts

Requests time out according to `Config::timeout`.

```rust
let mut config = Config::from_env();
config.timeout = std::time::Duration::from_secs(120);
```

## JSON-RPC Errors

The CLI rejected a request. Examples:

- calling a control method before `start()`;
- sending an expired permission request id;
- setting an unsupported model.

Null-ID startup failures are not discarded. The SDK promotes canonical
initialization and authentication errors into
`Error::InitializationFailed` and `Error::AuthenticationRequired`, including
their safe machine-readable fields. A request-bound authentication failure is
promoted in the same way.

Blueprint's restricted profiles also return dedicated errors for contract
version mismatches, profile violations, output/event limits, unsupported child
network isolation, blocked non-offline inference destinations, malformed
protocol frames, and a missing terminal event. These errors are failures; the
SDK never converts them into a successful answer or run.

Setup RPC failures that match the closed public setup codes are returned as
`Error::LoginFailed { problem }`. The problem contains only the closed code,
safe message, and retryability; raw API bodies, stderr, device codes, and
credentials are not exposed.

## Agent Events

Agent loop failures may arrive as `error` events in the stream. Handle them in your event loop and show enough context for users to recover.

## Recovery Patterns

- Stop and restart the SDK after transport failures.
- Use `interrupt()` or `Run::abort()` for user cancellation.
- Keep final summaries honest when checks fail.
- Keep raw event JSON available for debugging advanced CLI behavior.
- Treat `RunStatus::Failed` and `RunStatus::Cancelled` as non-success terminal
  outcomes. Only an observed canonical completed terminal event produces
  `RunStatus::Completed`.
- Helpers that would otherwise hide the status, including `run_json` and
  `Run::json`, return `Error::RunTerminated` for failed or cancelled runs.
