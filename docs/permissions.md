# Permissions

The Autohand CLI asks before running shell commands, writing files, or taking sensitive actions. The SDK surfaces those requests as `permission_request` events.

## Recommended Default

Keep permission handling interactive unless your host has a clear trust boundary:

```rust
sdk.set_permission_mode("interactive").await?;
```

## Responding To Requests

```rust
if event.event_type == "permission_request" {
    let request_id = event.request_id().expect("permission request id");
    sdk.permission_response(request_id, "allow_once").await?;
}
```

Common decisions:

- `allow_once`
- `deny_once`

## Agent Helpers

```rust
agent.allow_permission(request_id).await?;
agent.deny_permission(request_id).await?;
```

## Production Guidance

- Show the tool name and description to the user.
- Deny by default when request context is missing.
- Avoid blanket approval for file writes or shell commands.
- Strip secrets from logs before attaching them to issues.
- Use plan mode for discovery before allowing writes.
