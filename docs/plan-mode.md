# Plan Mode

Plan mode keeps the agent in a read-only planning posture. Use it for discovery, architecture review, and implementation planning before allowing writes or commands.

## Enable Plan Mode

```rust
let mut sdk = AutohandSdk::new(Config::from_env().with_cwd("."));
sdk.start().await?;
sdk.set_plan_mode(true).await?;
```

## Two-Phase Workflow

1. Start in plan mode.
2. Ask the agent to inspect and produce a plan.
3. Stop and review the plan.
4. Disable plan mode for the approved implementation.
5. Handle permissions explicitly during execution.

```rust
sdk.set_plan_mode(true).await?;
// discovery prompt
sdk.set_plan_mode(false).await?;
// implementation prompt
```

Plan mode and permission mode are separate. Plan mode controls which tools are available; permission mode controls whether individual tool calls require approval.
