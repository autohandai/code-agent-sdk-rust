# Resumable step control

Stop conditions run in your Rust host after the CLI has completed a tool step
and persisted its results. A stopped run leaves the same CLI session available
for another prompt.

```rust,no_run
use autohand_sdk::{Agent, Config, PromptOptions, Result, RunStatus, is_step_count};

#[tokio::main]
async fn main() -> Result<()> {
    let mut agent = Agent::create(Config::from_env().with_cwd(".")).await?;
    let result = agent.run_with_options(
        "Inspect this project and summarize the important files.",
        PromptOptions {
            stop_when: vec![is_step_count(3)?],
            ..PromptOptions::default()
        },
    ).await?;
    for step in &result.steps {
        println!("Step {}: {:?}", step.step_number, step.tool_results);
    }
    if result.status == RunStatus::Stopped {
        let continued = agent.run("Continue using the saved tool results.").await?;
        println!("{}", continued.text);
    }
    agent.close().await
}
```

`Agent::send_with_options` returns a `Run` for incremental `next()` consumption.
`RunResult` includes `steps` alongside `text`, `events`, `id`, and `status`.
`RunStatus::Stopped` is distinct from `Completed`, `Failed`, and `Cancelled`.
Repeated `wait()` calls do not send another prompt; prior failures remain errors.

`is_step_count(n)` stops after at least `n` steps in the current prompt.
`has_tool_call("read_file")` stops after a step that used that tool. The vector of
conditions uses OR semantics, stopping evaluation at the first true result. An
empty vector disables host control. Helpers reject zero counts and blank names.

Use `StopCondition::from_fn` for inexpensive synchronous checks and
`StopCondition::new` for asynchronous decisions:

```rust
use autohand_sdk::StopCondition;

let stop_on_failure = StopCondition::new(|context| async move {
    Ok(context.steps.last().is_some_and(|step| {
        step.tool_results.iter().any(|result| !result.success)
    }))
});
```

`StopConditionContext.steps` shares an ordered snapshot through `Arc`. Normal
evaluation does not copy the entire history for each callback. If a host retains
an older snapshot, subsequent steps preserve it through copy-on-write.

The SDK sends only `stopWhen: {mode: "host"}` over RPC. The CLI emits
`autohand.stepEnd`; Rust evaluates callbacks and answers `autohand.stepDecision`.
Notifications continue while predicates await host input. Step payloads and
decision responses are validated; callback errors first request a stop and wait
for the terminal event. Panics, invalid messages, or rejected decisions settle
the turn before surfacing an error.

All prompt APIs share the same turn queue. `AutohandSdk::prompt` now waits for
the terminal turn and returns the original RPC response; `stream_prompt` returns
events. Closing a raw stream aborts and drains a submitted turn. An unsubmitted
stream can be dropped without touching the CLI. Unresponsive cleanup retires
the process after a two-second drain deadline.

`Run::abort` and dropping an unfinished active `Run` preserve Rust's existing
process-tree termination behavior. Start a new SDK after aborting an active run.
Cancelling a queued run leaves the active process alone. Use stop conditions
when you want a resumable pause. JSON helpers continue to reject non-completed
runs with `Error::RunTerminated`; Blueprint answer-only and setup-only profiles
retain their separate restrictions.

The CLI must implement the step-control protocol. The opt-in runtime check uses
local authentication and Autohand AI HTTP mocks, reads a real file, stops, then
verifies the persisted tool result reaches the next model request:

```bash
AUTOHAND_TEST_CLI_PATH=/path/to/autohand cargo test --test harness_step_control -- --nocapture
```

This fixture supplies an explicit provider configuration and context window.
It verifies the actual provider/tool/step boundary; provider selection solely
through SDK options and distribution freshness are separate checks.
