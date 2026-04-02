# complex (cx)

A hierarchical issue tracker for agents. Local-first, git-native, CLI-only.

```
$ cx surface
izeU   Write tests         —
9us3   README              —
```

## Install

```bash
cargo install complex
```

Or from source:

```bash
cargo install --path .
```

## Quick start

```bash
# Initialize in your project
cd my-project
cx init                        # creates .complex/ (use --ephemeral to .gitignore it)

# Create a root complex and decompose it
cx add "User authentication"
# created  a3F2  User authentication

cx new a3F2 "Implement JWT tokens"
cx new a3F2 "Set up OAuth provider"
cx new a3F2 "Write auth middleware"

# Surface tasks so agents can claim them
cx surface bX7c
cx surface cY8d
cx surface dZ9e

# Declare ordering when it matters
cx block bX7c dZ9e             # JWT must finish before middleware

# Claim, work, integrate
cx claim bX7c --as agent-1
# ... do the work ...
cx integrate bX7c              # done — dependents are auto-surfaced

# Review state
cx status                      # tree + ready nodes
cx tree                        # full hierarchy
cx therapy                     # stale or stuck work
```

## Concepts

**States** — every node moves through four states:

```
latent → ready → claimed → integrated
```

Newly created children start `latent`. `cx surface <id>` promotes to `ready`.
`cx claim` moves to `claimed`. `cx integrate` marks done and unblocks dependents.

**Shadow** — a boolean flag orthogonal to state. A node can be claimed *and*
shadowed (claimed but stuck). `cx shadow <id>` / `cx unshadow <id>`.

**Parts** — agents or humans that claim work. Set via `cx claim --as <name>`
or the `CX_PART` environment variable.

**IDs** — flat 4-character base62 strings. Hierarchy is tracked via an explicit
`parent` field, not the ID itself:

```
a3F2   ← root complex
bX7c   ← child task  (parent: a3F2)
Qd4e   ← grandchild  (parent: bX7c)
```

Commands accept a short suffix when unambiguous: `cx show bX7c`.

**Edges** — relationships between nodes:

| Edge | Effect | Command |
|------|--------|---------|
| `blocks` | Target cannot become ready until source integrates | `cx block a b` |
| `waits-for` | Target waits for all children of source to integrate | (internal) |
| `discovered-from` | Informational — a was found while working on b | `cx discover a b` |
| `related` | Informational — loose association | `cx relate a b` |

**Tags** — labels that propagate from parent to children automatically.
A child's effective tags = own tags + all ancestor tags. Use `cx tag`/`cx untag`/`cx tags`.

**Comments** — append-only threads on any node. Useful for proposals, reviews,
or recording decisions. `cx comment <id> "text"` / `cx comments <id>`.

**Metadata** — arbitrary JSON blob on any node, ignored by complex.
Orchestrators and external tools use this for workflow hints, priorities, etc.
`cx meta <id> '{"priority": 1}'` / `cx meta <id>` to read.

## Storage

```
.complex/
  nodes/                    ← one JSON file per live node (metadata + edges)
    a3F2.json
    bX7c.json
  events/                   ← per-invocation event files
    2026-04-02T17-20-02.jsonl
  events.jsonl              ← legacy append-only audit log
  issues/                   ← markdown body per node
    a3F2.md
    bX7c.md
  issues/
    bX7c.comments.json      ← comment threads
  archive/
    nodes/                  ← one JSON file per archived node
      c5D8.json
    c5D8.md                 ← archived bodies
```

Each node is its own file, so parallel branches editing different nodes never
conflict. On every invocation `cx` loads the node files into an in-memory
SQLite database, runs the query or mutation, then writes back. No daemon, no
server, no port.

`git log -- .complex/` tells the full story of what happened.

## Commands

| Command | Description |
|---------|-------------|
| `cx init [--ephemeral]` | Initialize `.complex/` (`--ephemeral` adds to `.gitignore`) |
| `cx status` | Tree + ready nodes (quick overview) |
| `cx add <title>` | Create a root complex (`--by`, `--tag`, `-F <file>`) |
| `cx new <parent> <title>` | Create a child node (`--by`, `--tag`, `-F <file>`) |
| `cx surface` | List ready nodes |
| `cx surface <id>...` | Promote latent → ready (`--all` for all unblocked) |
| `cx claim <id>` | Claim a node (`--as <part>` or `$CX_PART`) |
| `cx unclaim <id>` | Release claim → ready |
| `cx integrate <id>` | Mark done, unblock dependents |
| `cx archive` | Archive all integrated nodes (`--ids a,b` for specific ones) |
| `cx unarchive <id>` | Restore archived node to graph |
| `cx rm <id>` | Remove/discard a node |
| `cx shadow` | List shadowed nodes |
| `cx shadow <id>` | Flag as shadowed |
| `cx unshadow <id>` | Clear shadow flag |
| `cx show <id>` | Node detail: title, body, edges, children, comments |
| `cx tree [id]` | Full hierarchy with states |
| `cx rename <id> <title>` | Rename a node |
| `cx edit <id>` | Set body (TTY → `$EDITOR`, piped → stdin, `--file`, `--body`) |
| `cx move <id> <new-parent>` | Reparent a node and its children (`--root` to promote) |
| `cx block <a> <b>` | a blocks b (cycle-safe) |
| `cx unblock <a> <b>` | Remove blocks edge |
| `cx relate <a> <b>` | Non-blocking association |
| `cx discover <a> <b>` | a was discovered while working on b |
| `cx tag <id> <tag>` | Add a tag |
| `cx untag <id> <tag>` | Remove a tag |
| `cx tags [id]` | Show effective tags, or list all tags in use |
| `cx find <query>` | Search by title (case-insensitive) |
| `cx list` | All nodes (`--state`, `--filed-by`, `--tag` to filter) |
| `cx meta <id> [json]` | Read or write metadata |
| `cx comment <id> [body]` | Append comment (`--tag`, `--as`, `--file`, `--edit`, `--rm`) |
| `cx comments <id>` | Read comment thread (`--tag` to filter) |
| `cx parts` | Claimed nodes grouped by part |
| `cx therapy` | Stale, shadowed, and stuck nodes |
| `cx log` | Recent events (`--limit N`, default 20) |
| `cx agent` | Print the agent guide |

All mutation commands accept `--reason "..."` (stored in events log).
All commands support `--json` for machine-readable output.

## Environment

| Variable | Description |
|----------|-------------|
| `CX_PART` | Identity of the current agent or user |
| `CX_FILED_BY` | Default `--by` value (convention: `project:agent`) |
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

`cx therapy --json` surfaces stuck or stale work — useful for orchestrators.

`cx discover <new> <existing>` records that a task was found while working
on another — preserving the reasoning chain without blocking anything.

Run `cx agent` to print a full agent guide suitable for system prompts.

## Claude Code integration

Add these patterns to `~/.claude/settings.json` to auto-approve `cx` commands:

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
