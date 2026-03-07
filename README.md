# complex

A hierarchical issue tracker for agents.

Named after the Jungian *complex* — a cluster of charged, associated material with a telos.
That's what a project really is.

```
cx surface
JMpv.izeU   Write tests         —
JMpv.9us3   README              —
```

## The model

Issues have **depth**. They move through states:

```
latent → ready → claimed → integrated
```

A node can also be **shadowed** — blocked or avoided — without changing its state.
Shadow is orthogonal to depth.

**Parts** are agents (or humans) that claim work. Any node without a blocker
and in `ready` state is available in parallel. Dependencies must be declared
explicitly with `cx block`.

**Complexes** are root-level clusters. Humans typically create these.
Agents decompose them into tasks, claim leaves, and integrate upward.

## IDs

IDs are dot-separated base62 segments. The hierarchy is encoded in the ID itself:

```
a3F2              ← root complex
a3F2.bX7c         ← child task
a3F2.bX7c.Qd4e   ← grandchild subtask
```

Parent is always derivable by stripping the last segment. No sequence numbers,
no locks — safe for concurrent agents creating nodes simultaneously.

Commands accept short IDs (leaf segment) when unambiguous:
`cx claim bX7c` resolves to `a3F2.bX7c`.

## Storage

`complex` lives inside your project as a `.complex/` directory tracked by git:

```
.complex/
  graph.json      ← active nodes: states, parts, edges
  issues/         ← markdown body for each node
  archive/        ← integrated nodes (out of the way, still in history)
```

On every invocation `cx` reads `graph.json` and all issue bodies into an
**in-memory SQLite** database, runs the query or mutation, then writes back.
No daemon. No server. No port. Cold start < 50ms for projects up to 1000 nodes.

`git log -- .complex/` tells the full story of what happened.

## Install

```bash
cargo install --path .
```

Or build and put `cx` on your PATH:

```bash
cargo build --release
cp target/release/cx ~/.local/bin/
```

## Usage

### Start a project

```bash
cd my-project
cx init
```

### Human adds a complex (high-level intent)

```bash
cx add "User authentication"
# created  a3F2  User authentication
```

### Decompose into tasks

```bash
cx new a3F2 "Implement JWT tokens"
cx new a3F2 "Set up OAuth provider"
cx new a3F2 "Write auth middleware"
```

### Surface tasks for agents to pick up

```bash
cx surface a3F2.bX7c   # latent → ready
cx surface a3F2.cY8d
cx surface a3F2.dZ9e
```

### Agent workflow

```bash
# Find available work
cx surface --json

# Claim a task
cx claim bX7c --as agent-1
# or set CX_PART=agent-1 and just: cx claim bX7c

# Declare a dependency
cx block bX7c dZ9e   # JWT must integrate before middleware is ready

# Mark done
cx integrate bX7c
```

### Review

```bash
cx tree              # full hierarchy with states
cx parts             # what each agent has claimed
cx therapy           # stale or stuck nodes needing attention
cx list              # all nodes
cx list --state ready
cx show bX7c         # node detail + body
```

## Commands

| Command | Description |
|---|---|
| `cx init` | Initialize `.complex/` in current directory |
| `cx add <title>` | Create a new root complex |
| `cx new <parent> <title>` | Create a child node |
| `cx surface` | List ready nodes |
| `cx surface <id>` | Promote latent → ready |
| `cx claim <id>` | Claim a ready node (`--as <part>` or `$CX_PART`) |
| `cx unclaim <id>` | Release a claim → ready |
| `cx integrate <id>` | Mark done, move to archive |
| `cx rm <id>` | Remove/discard a node (not integrate) |
| `cx shadow` | List shadowed nodes |
| `cx shadow <id>` | Flag a node as shadowed |
| `cx unshadow <id>` | Clear shadow flag |
| `cx show <id>` | Node detail: title, body, edges, children |
| `cx tree [id]` | Full hierarchy with states |
| `cx find <query>` | Search nodes by title (case-insensitive) |
| `cx list [--state <s>]` | All nodes, optionally filtered by state |
| `cx parts` | Claimed nodes grouped by part |
| `cx therapy` | Stale, shadowed, and orphan body files |
| `cx rename <id> <title>` | Rename a node's title |
| `cx edit <id>` | Open body in `$EDITOR` |
| `cx block <a> <b>` | `a` blocks `b` — cycle-safe |
| `cx unblock <a> <b>` | Remove a blocks edge |
| `cx relate <a> <b>` | Non-blocking association |
| `cx discover <a> <b>` | `a` was found while working on `b` |

All mutation commands accept `--reason "..."` to record rationale (stored in
`events.jsonl` and `meta._reason`). All commands support `--json` for
machine-readable output.

## Environment

| Variable | Description |
|---|---|
| `CX_PART` | Identity of the current agent or user |
| `CX_DIR` | Override `.complex/` location |
| `EDITOR` | Used by `cx edit` |

## For agents

The minimal agent loop:

```
1. cx surface --json          → find available work
2. cx claim <id> --as <name>  → take ownership
3. ... do the work ...
4. cx integrate <id>          → done, unblocks dependents
5. git add .complex/ && git commit
```

`cx therapy --json` surfaces work that is stuck or stale —
useful for an orchestrator deciding where to intervene.

`cx discover <new> <existing>` records that a task was found
while working on another — preserving the reasoning chain in
the graph without blocking anything.

## Claude Code integration

To let Claude Code run `cx` commands without prompting for approval each time,
add these patterns to `~/.claude/settings.json`:

```json
{
  "permissions": {
    "allow": [
      "Bash(cx *)",
      "Bash(cat <<*| cx *)"
    ]
  }
}
```

The first pattern covers direct `cx` calls. The second covers heredoc pipes
used to set issue bodies (e.g. `cat <<'BODY' | cx new <id> "title" --body -`).

## Why not markdown files?

Markdown task lists are linear and not queryable. Two agents editing the same
file produce merge conflicts. There is no machine-readable way to ask
"what can I work on right now?"

## Why not a hosted issue tracker?

No network dependency. No API keys. No webhooks. Lives in the repo,
ships with the code, travels with the clone.

## Why not [beads](https://github.com/steveyegge/beads)?

Beads is excellent and was a direct inspiration. `complex` is the 20% that
covers 80% of agent workflows: hierarchy, state, blocking dependencies,
parallel-by-default, and git-native storage. No Dolt, no federation,
no molecules, no mail system.
