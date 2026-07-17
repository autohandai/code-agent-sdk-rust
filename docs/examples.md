# Examples

The Rust examples mirror the TypeScript SDK examples and are intended to teach one workflow at a time.

Run one with:

```bash
cargo run --example 01-hello-agent
```

Examples:

- `01-hello-agent.rs`: first prompt.
- `02-streaming-query.rs`: stream message events.
- `03-code-reviewer.rs`: inspect repository files.
- `04-bash-command.rs`: command-oriented prompt with permissions.
- `05-file-editor.rs`: file-editing workflow.
- `06-prompt-skills.rs`: skill-aware prompting.
- `07-direct-skills.rs`: preconfigured skills.
- `08-memory-management.rs`: memory save and recall pattern.
- `10-multi-tool-reasoning.rs`: inspect, test, and summarize.
- `13-permissions.rs`: explicit permission mode.
- `20-sdlc-discovery-plan.rs`: plan-only discovery.
- `21-sdlc-gated-implementation.rs`: plan then execute.
- `22-sdlc-release-readiness.rs`: release gate.
- `23-system-prompts.rs`: appended system instructions.
- `24-high-level-agent.rs`: `Agent` and `Run`.
- `25-structured-json.rs`: typed JSON output.
- `27-autoresearch-ledger.rs`: typed persisted experiments, replay, rescoring, Pareto, and pruning.

Live examples require an authenticated Autohand CLI.
