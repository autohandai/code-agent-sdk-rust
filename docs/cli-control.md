# Conversation, Browser Handoff, And Auto-Mode Control

These typed methods are available on both `AutohandSdk` and `Agent`.

## Reset A Conversation

Call `reset()` to clear the active conversation and start a new session. The
returned `ResetResult::session_id` is the CLI-assigned session identifier.
