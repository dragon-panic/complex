## Problem

`cx integrate` has inconsistent behavior around archiving:

- Some tasks get removed from graph.json entirely (MAVL, 9i43, etc.)
- Others stay in graph.json with state=integrated (TjMl)
- All get their body files moved to `.complex/archive/`

This means completed plan tasks disappear from `cx tree` immediately after
integration. There's no window where you can see a plan's completed work
as "done" — it goes straight from claimed to invisible.

## Expected behavior

Integration should be a two-phase process:

1. **`cx integrate <id>`** — marks the task as done, unblocks dependents,
   but keeps the node in the tree with state=integrated. The task is visible
   in `cx tree` and `cx list --state integrated`. Body file stays in
   `.complex/issues/`.

2. **`cx archive <id>`** (or `cx archive --plan <plan-id>`) — moves the
   node out of the active tree and body to `.complex/archive/`. No longer
   visible in `cx tree` unless you pass `--archived`.

This gives the manager (and board) a window to review completed work
before it disappears. A plan's tasks stay visible as integrated until
someone explicitly archives them — typically after the plan completion
checkpoint.

## Additional: bulk archive

`cx archive --plan <plan-id>` should archive all integrated children of a
plan node in one operation. Useful after a plan completion checkpoint when
you want to clean up the tree.

## Current workaround

None. Tasks vanish on integrate and you have to look at the archive
directory or git history to see what was done.
