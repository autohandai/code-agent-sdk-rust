# SDLC Workflows

This repository supports the same inspectable SDK workflow as the TypeScript SDK.

Read the full guide: [docs/sdlc-workflows.md](./docs/sdlc-workflows.md)

The short version:

1. Use plan mode for discovery.
2. Review the plan outside the agent loop.
3. Disable plan mode only after approval.
4. Execute with explicit permission handling.
5. Run the release gates before publishing.

```bash
cargo fmt --check
cargo test
cargo check --examples
```
