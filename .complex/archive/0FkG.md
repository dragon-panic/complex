## Problem
`cx edit` is the only mutation command that doesn't emit an event to events.jsonl.
All other mutations (create, rename, claim, shadow, integrate, rm, etc.) emit events.

## Fix
Add `emit(&root, "edit", &resolved, None, None, None)` after a successful body write
in `cmd_edit`.
