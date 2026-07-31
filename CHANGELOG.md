# Changelog

## Unreleased

### Changed

- **Breaking:** `RunResult::status` is now the closed `RunStatus` enum instead
  of a string. It reflects the observed terminal event and no longer defaults
  every run to `"completed"`.

### Added

- Typed community skill registry discovery and installation.
- Typed MCP server, tool, and configuration discovery.
- A deterministic three-metric startup performance gate and baseline.
- A versioned Blueprint answer-only contract with closed artifact classes,
  strict typed JSON results, executed CLI identity/provenance, bounded capture,
  cleared child environments, and enforceable deny-all egress.
- A mutually exclusive setup-only Autohand device-authorization contract with
  an opaque session handle and safe begin, poll, and cancel results.
- Typed terminal run statuses, typed startup/authentication failures, and Rust
  1.76 consumer compatibility proof.

### Fixed

- Made startup transactional and verified CLI readiness before publishing the
  started lifecycle state.
- Shared lifecycle state across cloned SDK handles.
- Ensured dropping the final SDK handle releases and terminates its child
  process rather than retaining it through reader tasks.
- Removed pending requests after write failures and resolved all pending
  requests immediately when stdout closes.
- Removed pending request IDs when a stream receiver is dropped and cancels its
  in-flight prompt future.
- Kept streams open through message and turn boundaries until canonical
  `agentEnd` or an error arrives.
- Terminated the complete CLI process tree on timeout, abort, dropped active
  runs/setup sessions, and final SDK shutdown.
- Reported event-stream lag and missing terminal events as errors instead of
  silently losing state or claiming completion.
