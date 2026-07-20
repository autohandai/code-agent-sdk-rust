# Startup Performance

`transport::tests::startup_budgets` guards the work controlled by this SDK with
a deterministic local JSON-RPC fixture. Each metric uses five warmup iterations
and 50 measured samples. The test reports median and p95 latency and fails if
any p95 reaches 50 ms.

- `publicImportMs`: the first public `autohand_sdk::initialize()` call timed
  inside a fresh Rust test process, excluding Rust runtime process boot. This
  idempotent hook performs the crate's eager runtime initialization.
- `sdkStartReturnMs`: elapsed time for the public `AutohandSdk::start` API,
  including its readiness `getState` request.
- `fixtureSpawnToFirstRpcMs`: elapsed time to spawn the deterministic fixture
  and complete a successful `getState` through the transport.

Baseline captured on 2026-07-20:

| Metric | Median | p95 | Budget |
| --- | ---: | ---: | ---: |
| `publicImportMs` | 0.003417 ms | 0.006750 ms | < 50 ms |
| `sdkStartReturnMs` | 5.668250 ms | 6.774875 ms | < 50 ms |
| `fixtureSpawnToFirstRpcMs` | 5.564083 ms | 6.308125 ms | < 50 ms |

Run the gate directly:

```bash
cargo test --lib startup_budgets -- --nocapture
```

The test emits one machine-readable JSON object with top-level `language`,
`budgetMs`, `metrics`, and `passed` fields. Each metric contains `samples`,
`medianMs`, `p95Ms`, `maxMs`, and `passed`. The fixed protocol is five warmups
and 50 measured samples.

This benchmark isolates wrapper overhead. A real Autohand CLI may additionally
perform provider authentication, network access, model loading, and other
environment-specific readiness work; those live measurements are deliberately
reported separately from the deterministic 50 ms gate.
