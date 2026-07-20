# Event Streaming

`stream_prompt()` starts a prompt and returns events as they arrive.

## Basic Pattern

```rust
let mut events = sdk
    .stream_prompt("Explain closures in one sentence.", PromptOptions::default())
    .await?;

while let Some(event) = events.recv().await {
    let event = event?;
    if let Some(delta) = event.text_delta() {
        print!("{delta}");
    }
}
```

Dropping the receiver cancels that stream's in-flight prompt wait. The
transport also removes its pending request ID immediately, so abandoned streams
do not accumulate request state in long-lived hosts.

## Event Types

- `message_update`: token or text delta.
- `message_end`: final message content.
- `tool_start`: a tool started.
- `tool_update`: streaming tool output.
- `tool_end`: a tool completed.
- `permission_request`: host approval is required.
- `error`: agent or transport error.

## Handling Permissions While Streaming

```rust
while let Some(event) = events.recv().await {
    let event = event?;
    if event.event_type == "permission_request" {
        if let Some(request_id) = event.request_id() {
            sdk.permission_response(request_id, "allow_once").await?;
        }
    }
}
```

Production hosts should route permission requests to a human, policy engine, or trusted automation boundary.

## Collecting Final Text

Use `Agent` and `Run` when you want streaming and a final result:

```rust
let mut run = agent.send("Summarize this repository.").await?;
while let Some(event) = run.next().await {
    println!("{:?}", event?);
}
let result = run.wait().await?;
println!("{}", result.text);
```
