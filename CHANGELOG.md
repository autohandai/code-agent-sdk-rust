# Changelog

## Unreleased

### Added

- Typed community skill registry discovery and installation.
- Typed MCP server, tool, and configuration discovery.
- A deterministic three-metric startup performance gate and baseline.

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
