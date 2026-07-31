# Security Policy

## Reporting Security Issues

Please do not open public GitHub issues for security vulnerabilities.

Email reports to [security@autohand.ai](mailto:security@autohand.ai). Include:

- A description of the vulnerability.
- Steps to reproduce it.
- Affected SDK version or commit.
- Whether the issue involves the Autohand CLI, provider credentials, tool permissions, or local file access.

We will review the report and coordinate next steps privately.

## Sensitive Information

When filing normal bugs or feature requests:

- Remove API keys and provider tokens.
- Redact local file paths when needed.
- Do not paste private repository contents unless they are required and safe to share.
- Be careful with agent logs because they can include prompts, tool output, or file snippets.

## Scope

This policy covers the Rust SDK repository and its interaction with the Autohand CLI JSON-RPC mode. Security issues in the CLI itself may also affect SDK users; include that context in your report if relevant.

## Blueprint Restricted Profiles

The Blueprint answer-only profile accepts only the versioned classified answer
contract. It clears the child environment, rejects general prompt/RPC and
user-supplied CLI argument escape hatches, validates passive runtime facts and
the executed CLI identity, bounds all captured data, and runs the CLI process
tree with deny-all network access. If that network policy cannot be enforced,
startup fails closed.

The setup-only profile is a separate process and RPC allowlist for Autohand
device authorization. It receives no workspace evidence or question. Private
device state remains inside the CLI/SDK session; callers receive only the user
code, the validated complete Autohand sign-in URL, expiry, poll interval, and
safe typed status. Dropping an unfinished session terminates its process tree.

Neither profile fabricates an answer, login challenge, or authorized outcome.
Production success is returned only from a validated CLI response.
