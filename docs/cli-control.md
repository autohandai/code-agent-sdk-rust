# Conversation, Browser Handoff, And Auto-Mode Control

These typed methods are available on both `AutohandSdk` and `Agent`.

## Reset A Conversation

Call `reset()` to clear the active conversation and start a new session. The
returned `ResetResult::session_id` is the CLI-assigned session identifier.

## Create A Browser Handoff

Call `create_browser_handoff(params)` to create a continuation token for the
active session. `extension_id` and `install_url` are optional; the result
contains the token, session and workspace identifiers, timestamps, and URL.

## Attach A Browser Handoff

Call `attach_browser_handoff(params)` with a token. A successful result may
include the restored session ID, workspace root, and message count.

Use `attach_latest_browser_handoff()` when the newest unexpired handoff should
be selected without supplying a token.

## Start Auto-Mode

Call `start_automode(params)` with a required prompt and optional iteration,
completion, worktree, checkpoint, runtime, and cost limits. A successful result
contains the accepted auto-mode session ID; execution continues in the CLI.

## Inspect Auto-Mode Status

Call `get_automode_status()` for the live `active` and `paused` flags plus the
optional persisted state, iteration and file counters, branch, and checkpoint.

## Pause Auto-Mode

Call `pause_automode()` to pause the active session. CLI business failures are
returned as `AutomodePauseResult { success: false, error: Some(...) }`.

## Resume Auto-Mode

Call `resume_automode()` to continue a paused session. The result reports
business success or an optional CLI error without converting it to transport failure.

## Cancel Auto-Mode

Call `cancel_automode(params)` to stop the active session. `reason` is optional;
the result reports business success or an optional CLI error.
