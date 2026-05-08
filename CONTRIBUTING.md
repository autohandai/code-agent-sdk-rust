# Contributing to the Autohand Code Agent SDK for Rust

Thanks for helping improve the Rust SDK. This repository is open source and sits beside the public Autohand Code CLI and the other Agent SDK language packages.

## Before You Start

- Read the Agent SDK docs: https://autohand.ai/docs/agent-sdk/
- Search existing issues before opening a new one.
- Keep public API changes small, typed, and ergonomic.
- Do not commit secrets, provider keys, private logs, or local machine paths.

## Development Setup

```bash
git clone https://github.com/autohandai/code-agent-sdk-rust.git
cd code-agent-sdk-rust
cargo test
cargo check --examples
```

Live examples require an authenticated Autohand CLI. Set `AUTOHAND_CLI_PATH` if you want to test against a local CLI build:

```bash
export AUTOHAND_CLI_PATH=/path/to/autohand
cargo run --example 01-hello-agent
```

## Validation

Run the full local gate before opening a pull request:

```bash
cargo fmt --check
cargo test
cargo check --examples
```

The unit tests use a fake CLI transport where possible so contributors can validate core SDK behavior without model credentials.

## Pull Requests

Good SDK pull requests usually include:

- A focused API or behavior change.
- Tests for transport, JSON parsing, or configuration behavior.
- Updated examples when public APIs change.
- Updated docs when behavior, setup, or workflows change.

## Commit Style

Use Conventional Commits, following the same style as Autohand Code CLI:

```text
feat: add run cancellation helper
fix: avoid losing final stream events
docs: document plan mode workflow
test: cover permission response flow
```

## Review Principles

We optimize for humans using the API:

- Prefer clear names over clever names.
- Keep direct CLI escape hatches for advanced users.
- Make permissions visible to host applications.
- Keep examples copy-pasteable.
- Keep docs honest about beta status and runtime requirements.

## Community

By participating, you agree to follow the repository [Code of Conduct](./CODE_OF_CONDUCT.md).
