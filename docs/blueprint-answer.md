# Blueprint Answer And Setup-Only Contracts

Blueprint uses two closed, mutually exclusive runtime profiles. Neither profile
is a general-purpose agent session.

## Answer-Only

Create the profile with
`Config::with_answer_only_profile(AnswerOnlyProfile::blueprint())`. The SDK
starts the CLI with:

```text
--mode rpc --answer-only --restricted --client-context blueprint
```

Startup calls only `autohand.runtimeInspect`. The SDK verifies contract version
1, the complete executed CLI identity, Blueprint client context, restricted
permissions, disabled tools/hooks/MCP/memory/session persistence, and the
provider, model, authentication state, and inference destination.

Only `in_process` and `local_subprocess` inference may receive classified
artifacts. `local_service`, `hosted`, and `opaque` destinations return
`Error::InferenceDestinationBlocked` before the answer request is sent.

`run_answer` accepts a `ClassifiedAnswerEnvelope` and `StrictJsonSchema`. The
envelope carries the exact closed artifact classes, a lowercase SHA-256 policy
identity, and bounded classified content. The response is one strict framed
JSON object: unknown fields, trailing data, schema violations, contract
mismatches, and provenance changes fail the call. `run_json` remains available
for compatibility but is not suitable for this strict security boundary.

The SDK validator intentionally supports a small enforceable JSON Schema
subset: strict objects, arrays, strings, integers, numbers, booleans, null,
enums, required properties, and item/string length bounds. Unsupported
keywords are rejected instead of being silently ignored. The root must be an
object with an explicit, unique `required` list.

The canonical answer schema and golden vectors are:

- `schema/blueprint-answer-contract-v1.schema.json`
- `schema/blueprint-answer-contract-v1.valid.json`
- `schema/blueprint-answer-contract-v1.invalid.json`

## Child Isolation

Restricted profiles clear the inherited environment and restore only minimal
process requirements plus names explicitly added with `allow_environment`.
Blueprint production integrations should leave the explicit allowlist empty
unless a reviewed runtime dependency requires a value.

The restricted child also starts in the platform temporary directory rather
than inheriting Blueprint's workspace current directory.

Answer-only starts the complete child process tree with deny-all network access:

- macOS uses the system sandbox profile with all network operations denied.
- Linux requires Bubblewrap and an unshared network namespace.
- unsupported platforms or unavailable enforcement return
  `Error::NetworkPolicyUnavailable`.

The answer process cannot be reused for setup or general interactive RPC.

## Setup-Only Autohand Sign-In

Create setup with
`Config::with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization())`.
It starts the CLI with:

```text
--mode rpc --setup-only --restricted --client-context blueprint
```

The only allowed methods are:

- `autohand.login.begin`
- `autohand.login.poll`
- `autohand.login.cancel`

Begin classifies traffic as `autohand_device_authorization`. The paired CLI
permits only the versioned Autohand device-authorization API host/path contract;
the SDK supplies no workspace, repository, question, evidence, provider probe,
or model request.

`LoginChallenge` exposes only the user code, a complete
`https://autohand.ai/signin` URL, expiry, poll interval, and an opaque,
non-serializable `LoginSession`. The URL must contain exactly the signed
`continue` value and matching `user_code`; fragments, extra keys, other hosts,
and non-HTTPS URLs are rejected. Device codes, credentials, raw API responses,
and stderr are never public fields. `Authorized` is accepted only after the CLI
reports that credential persistence completed.

The canonical setup schema and vectors are:

- `schema/blueprint-setup-contract-v1.schema.json`
- `schema/blueprint-setup-contract-v1.valid.json`
- `schema/blueprint-setup-contract-v1.invalid.json`

Dropping an unfinished setup session aborts its full CLI process tree. Explicit
cancel waits for a canonical `cancelled` response.

## Bounds And Lifecycle

Structured input/output, transport stdout/stderr, event count, and event bytes
all have hard limits. Limit violations, malformed frames, timeout, abort,
dropped active runs, and shutdown are errors and terminate the process tree.
Unix uses a dedicated process group; Windows uses a kill-on-close Job Object.
Restricted callers may tighten the version-1 limits but cannot relax them.

The SDK never creates mock answers, fabricated login challenges, or synthetic
success in production code. Tests use subprocess fixtures only to prove the
wire and lifecycle behavior.
