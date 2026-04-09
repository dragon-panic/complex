pub const AGENT_GUIDE: &str = r#"# complex — agent guide

This project uses `complex` (cx) for task tracking. You are a **part** —
an agent that claims, works, and integrates tasks.

## Workflow

1. Find available work:   cx surface --json
2. Claim a task:          cx claim <id> --as <your-name>
3. Do the work
4. If you discover a sub-task while working:
                          cx new <parent-id> <title>
                          cx discover <new-id> <current-id>
5. Mark done:             cx integrate <id>
6. Commit:                git add .complex/ && git commit -m "integrate(<id>): <title>"

Tasks are **parallel by default**. Only an explicit `cx block <a> <b>` creates
ordering. Run `cx surface` at any time — it only shows tasks with no open blockers.

## Commands you will use

```
cx status --json                  tree + ready nodes (quick overview)
cx surface --json                 ready tasks (no open blockers)
cx surface --all --json           promote all latent tasks with no blockers to ready
cx claim <id> --as <name>         take ownership (or set CX_PART env var)
cx unclaim <id>                   release if you cannot complete it
cx integrate <id>                 mark done → archive; auto-surfaces any newly unblocked latent tasks
                                  JSON includes "newly_surfaced": [...] when tasks are unblocked
cx rm <id>                        remove/discard a node (not integrate)
cx new <parent-id> <title>        create a child task under a parent
cx add <title> --body "markdown"  create with body in one shot (also works on cx new)
cx add <title> --by <who>        record who filed this (or set CX_FILED_BY)
cx discover <new-id> <source-id>  record task found while working on source
cx find <query>                   search nodes by title (case-insensitive)
cx tag <id> <tag>                 add a tag to a node
cx untag <id> <tag>               remove a tag from a node
cx tags [id]                      show effective tags (own + inherited) or list all
cx rename <id> <new title>        rename a node
cx move <id> <new-parent>         reparent a node (and children) under a new parent
cx move <id> --root               promote a node to root level
cx shadow <id>                    flag as blocked/stuck
cx edit <id> --body "markdown"    update body non-interactively (or pipe: echo "md" | cx edit <id>)
cx comment <id> --tag proposal --file /tmp/plan.md   append a comment
cx comment <id> --tag review "PASS — looks good"     append with inline body
cx comment <id> --edit <timestamp> "new text"         edit a comment by timestamp
cx comment <id> --rm <timestamp>                      remove a comment
cx comments <id> --json           read the full comment thread
cx comments <id> --tag proposal   filter comments by tag
cx show <id> --json               full node detail: state, edges, body, children
cx tree --json                    full hierarchy with states (nested children)
cx parts --json                   what each part currently holds
cx therapy --json                 stale (claimed >24h), shadowed, and orphan body files
cx list --state claimed --json    all nodes in a given state
```

## Comments

Each node has an append-only comment thread — use it instead of overwriting
the body. The body is the spec; comments are the conversation about it.

```
cx comment <id> --tag proposal --file /tmp/plan.md    propose an approach
cx comment <id> --tag review "PASS — looks good"      review a proposal
cx comment <id> --tag code-review --file /tmp/cr.md   review the code
cx comments <id> --tag proposal --json                read the latest proposal
```

Tags are conventions, not a fixed enum: `proposal`, `review`, `code-review`,
`retro`, or omit for general discussion. Multiple comments can share a tag
(e.g. two `code-review` entries after a retry cycle).

`--as <who>` sets the author (falls back to `CX_FILED_BY`, then `"unknown"`).
Edit (`--edit <timestamp>`) and remove (`--rm <timestamp>`) reference comments
by their ISO 8601 timestamp.

## Rationale (--reason)

All mutation commands accept an optional `--reason` flag to record **why** you
are taking an action. The reason is stored in the node's `meta._reason` field
(quick lookup for orchestrators) and preserved in git history via commit messages.

```
cx claim <id> --as agent-1 --reason "has rust capability, no blockers"
cx shadow <id> --reason "tests failing, needs upstream fix in auth module"
cx unclaim <id> --reason "blocked on external API, releasing for others"
cx integrate <id> --reason "all tests pass, code reviewed"
cx surface <id> --reason "dependency resolved, ready for work"
cx unshadow <id> --reason "upstream fix landed"
```

Reason is always optional — omitting it never blocks an action.

## State model

```
latent → ready → claimed → integrated
                    ↕
                 shadowed  (flag — orthogonal to state)
```

**Important:** `cx claim` only works on `ready` nodes. You must `cx surface <id>`
a latent node before claiming it.

## IDs

Every node has a flat 4-character base62 ID (e.g. `a3F2`).
Parent-child relationships use an explicit `parent` field, not the ID.
Move (`cx move`) just updates the parent — IDs never change.

## Environment

  CX_PART      your identity — set this before claiming anything
  CX_FILED_BY  default --by value (convention: project:agent, e.g. seguro:ox)

## What to commit

After any cx mutation, stage and commit `.complex/`:
  git add .complex/ && git commit -m "claim(bX7c): implement JWT tokens"
  git add .complex/ && git commit -m "integrate(bX7c): implement JWT tokens"
"#;
