# Replayable Autoresearch Ledger

The Rust SDK exposes Autohand's persisted autoresearch engine through typed
JSON-RPC methods. A session proposes focused changes, evaluates each candidate,
records immutable measurements and decisions under `.auto/`, and can replay or
rescore earlier attempts without losing their original evidence.

Use a CLI build with autoresearch RPC support. Stopping a session pauses it;
the persisted ledger remains available to resume, inspect, replay, or prune.

## Start or resume

```rust
use autohand_sdk::{
    Agent, AutoresearchConstraint, AutoresearchConstraintOperator,
    AutoresearchOptimizationDirection,
    AutoresearchSamplingOptions, AutoresearchStartParams,
};

let started = agent
    .start_autoresearch(AutoresearchStartParams {
        objective: "Reduce checkout p95 latency without changing behavior".into(),
        max_iterations: Some(12),
        metric_name: Some("p95_ms".into()),
        metric_unit: Some("ms".into()),
        direction: Some(AutoresearchOptimizationDirection::Lower),
        measure_command: Some(
            "cargo test --release --bench checkout -- --sample-size 20".into(),
        ),
        checks_command: Some("cargo test".into()),
        files_in_scope: vec!["src/checkout/".into(), "src/cache/".into()],
        sampling: Some(AutoresearchSamplingOptions {
            min_samples: Some(3),
            max_samples: Some(7),
            confidence_threshold: Some(0.9),
        }),
        constraints: vec![AutoresearchConstraint {
            metric_name: "error_rate".into(),
            operator: AutoresearchConstraintOperator::LessThanOrEqual,
            threshold: 0.0,
        }],
        ..AutoresearchStartParams::default()
    })
    .await?;

if !started.success {
    return Err(autohand_sdk::Error::StructuredOutput(
        started.error.unwrap_or_else(|| "autoresearch could not start".into()),
    ));
}
```

Calling `start_autoresearch` for an existing paused session resumes its stored
configuration. The returned `instruction` is the loop prompt. Pass it to
`Agent::send` when your host owns each iteration. For the slash-command path,
use `agent.autoresearch(objective).await?`.

## Inspect persisted evidence

```rust
let status = agent.get_autoresearch_status().await?;
let history = agent.get_autoresearch_history().await?;

for attempt in &history.attempts {
    println!(
        "{} replayable={} pinned={} state={:?}",
        attempt.attempt_id,
        attempt.replayable,
        attempt.pinned,
        attempt.materialization,
    );
}
```

Each replayable attempt may contain its latest `AutoresearchEvaluationRecord`
and `AutoresearchDecisionRecord`. Samples retain all configured metrics;
aggregates use median and median absolute deviation.

## Replay, rescore, compare, and Pareto

Replay evaluates a candidate in an isolated worktree:

```rust
let original = agent
    .replay_autoresearch(AutoresearchReplayParams {
        attempt_id: candidate.attempt_id.clone(),
        evaluator: Some(AutoresearchEvaluatorMode::Original),
    })
    .await?;

let current = agent
    .replay_autoresearch(AutoresearchReplayParams {
        attempt_id: candidate.attempt_id.clone(),
        evaluator: Some(AutoresearchEvaluatorMode::Current),
    })
    .await?;
```

`AutoresearchRescoreParams` is an enum with constructors that make invalid
attempt/all combinations unrepresentable:

```rust
let one = agent
    .rescore_autoresearch(AutoresearchRescoreParams::attempt(&candidate.attempt_id))
    .await?;
let all = agent
    .rescore_autoresearch(AutoresearchRescoreParams::all())
    .await?;
```

Compare attempts and inspect the constraint-passing, non-dominated frontier:

```rust
let comparison = agent
    .compare_autoresearch(AutoresearchCompareParams {
        left_attempt_id: "attempt-1".into(),
        right_attempt_id: "attempt-2".into(),
    })
    .await?;
let pareto = agent.get_autoresearch_pareto().await?;
```

## Pin and prune safely

```rust
agent
    .pin_autoresearch(AutoresearchPinParams {
        attempt_id: candidate.attempt_id.clone(),
        pinned: true,
    })
    .await?;

let preview = agent
    .prune_autoresearch(AutoresearchPruneParams {
        dry_run: Some(true),
        yes: None,
    })
    .await?;

let applied = agent
    .prune_autoresearch(AutoresearchPruneParams {
        dry_run: Some(false),
        yes: Some(true),
    })
    .await?;
```

Always show the preview and request explicit confirmation before applying.

## Typed events and hooks

Autoresearch notifications use `SdkEvent` like every other stream event. Call
`event.autoresearch()` to decode either `AutoresearchEvent::Lifecycle` or
`AutoresearchEvent::Operation`:

```rust
if let Some(event) = event.autoresearch() {
    match event? {
        AutoresearchEvent::Lifecycle(event) => {
            println!("autoresearch {:?}: {}", event.phase, event.status_text);
        }
        AutoresearchEvent::Operation(event) => {
            println!("autoresearch {:?} {:?}", event.operation, event.phase);
        }
    }
}
```

`AutoresearchHookEvent` serializes to the CLI's canonical hook names from
`autoresearch:start` through `autoresearch:complete` and `autoresearch:error`.

See [`examples/27-autoresearch-ledger.rs`](../examples/27-autoresearch-ledger.rs)
for a runnable end-to-end workflow.
