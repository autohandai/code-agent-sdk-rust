# Conversation, Browser Handoff, And Auto-Mode Control

These typed methods are available on both `AutohandSdk` and `Agent`.

## Reset A Conversation

Call `reset()` to clear the active conversation and start a new session. The
returned `ResetResult::session_id` is the CLI-assigned session identifier.

## Create A Browser Handoff

Call `create_browser_handoff(params)` to create a continuation token for the
active session. `extension_id` and `install_url` are optional; the result
contains the token, session and workspace identifiers, timestamps, and URL.
