# complex — PRD

> A hierarchical, graph-structured issue tracker for agents.
> Named after the Jungian *complex*: a cluster of associated charged material
> that pulls behaviour toward it. That's what a project really is.

---

## Problem

Linear markdown task lists break down under agent workflows:

- No machine-queryable state ("what can I work on right now?")
- No dependency awareness ("what's blocking this?")
- No ownership model ("which agent has claimed this?")
- No parallel work discovery ("what can run concurrently?")
- No audit trail ("what happened while I was away?")

Existing tools (GitHub Issues, Linear, Jira) are human-first, network-dependent,
and carry enormous surface area. Beads (steveyegge/beads) is agent-first but
requires Dolt, a mail system, federation, molecule phases, and a chemistry
metaphor. We want the essential 20% that covers 80% of agent workflows.

---

## Solution

`complex` (`cx`) is a local-first, git-native, hierarchical issue tracker
that agents query and mutate via a simple CLI. It stores data as:

- A **file tree** (human-readable, git-diffable)
- Materialized into **in-memory SQLite** at runtime for query power
- Written back to disk on every mutation

Agents are the primary users. Humans input high-level *complexes* and review
state. The CLI is the only interface — no server, no network, no daemon.

---

## Core Concepts

### The Jungian Model

Issues are not tickets. They are *complexes* — clusters of charged,
unresolved material with a telos (individuation/integration).

| Jungian term  | `complex` meaning                        |
|---------------|------------------------------------------|
| Complex       | A root-level cluster of related work     |
| Part          | An agent (or agent role) claiming work   |
| Shadow        | A node that is blocked or being avoided  |
| Individuation | Completion — a node is *integrated*      |
| Therapy       | Reviewing stale, stuck, or shadowed work |

### State Axis (depth)

```
latent → ready → claimed → integrated
```

- `latent`     — exists but not yet surfaced; not available for claiming
- `ready`      — available; no unresolved blocking dependencies
- `claimed`    — a part (agent) is actively working it
- `integrated` — done; moved to archive

`shadowed` is a boolean flag orthogonal to state. A node can be
`claimed` and `shadowed` simultaneously (claimed but stuck).

### Parallel by Default

All sibling nodes are available in parallel unless a `blocks` edge
declares otherwise. Agents should claim whatever is `ready` without
waiting for coordination.

---

## ID Scheme

IDs encode hierarchy using dot-separated base62 segments:

```
a3F2              ← root complex
a3F2.bX7c         ← child task
a3F2.bX7c.Qd4e   ← grandchild subtask
```

- Each segment is 4 random base62 characters (0-9, A-Z, a-z)
- 62^4 = ~14.7M combinations per level — effectively zero collision risk
- Parent is always derivable by stripping the last segment
- No central counter; no locking; safe for concurrent agent creation
- Reparenting is rare and semantically significant enough to warrant a new ID

Human-facing commands accept the leaf segment alone if unambiguous:
`cx claim bX7c` resolves to `a3F2.bX7c`.

---

## File Layout

```
.complex/
  graph.json        ← active nodes: metadata, states, edges
  issues/           ← active markdown bodies (one file per node)
    a3F2.md
    a3F2.bX7c.md
    a3F2.bX7c.Qd4e.md
  archive/          ← integrated nodes
    archive.json    ← metadata for completed nodes
    c5D8.md
    c5D8.eF1a.md
```

### `graph.json` schema

```json
{
  "version": 1,
  "nodes": [
    {
      "id": "a3F2",
      "title": "Authentication",
      "state": "active",
      "shadowed": false,
      "part": null,
      "created_at": "2026-02-28T00:00:00Z",
      "updated_at": "2026-02-28T00:00:00Z"
    },
    {
      "id": "a3F2.bX7c",
      "title": "Implement JWT tokens",
      "state": "claimed",
      "shadowed": false,
      "part": "agent-backend",
      "created_at": "2026-02-28T00:00:00Z",
      "updated_at": "2026-02-28T00:00:00Z"
    }
  ],
  "edges": [
    { "from": "a3F2.bX7c", "to": "c5D8.eF1a", "type": "blocks" }
  ]
}
```

Parent-child relationships are **not** stored in edges — they are implicit
in the ID structure. The `edges` array stores only cross-tree relationships.

### `issues/<id>.md`

Free-form markdown. No required structure. Agents and humans write here.
`cx edit <id>` opens `$EDITOR` with a tempfile and reads it back atomically.

---

## Edge Types

Two classes: **workflow** (affect `cx surface`) and **association** (informational).

### Workflow edges

| Type        | Semantics                                              |
|-------------|--------------------------------------------------------|
| `blocks`    | Target cannot become `ready` until source is `integrated` |
| `waits-for` | Target waits for all children of source to integrate (fanout gate) |

### Association edges (non-blocking)

| Type               | Semantics                                            |
|--------------------|------------------------------------------------------|
| `discovered-from`  | This node was found while working on the source      |
| `related`          | Loose see-also link                                  |

---

## CLI — `cx`

All commands support `--json` for machine-readable output.

### Read commands

```
cx surface              list ready nodes (no blocking deps, state=ready)
cx parts                show claimed nodes grouped by part
cx shadow               show shadowed nodes
cx therapy              show stale/stuck nodes needing review
cx tree [id]            full hierarchy with states
cx show <id>            node detail: title, body, edges, history
cx log                  recent events (audit trail)
```

### Write commands

```
cx init                 initialize .complex/ in current directory
cx add                  create a new root complex (human-facing)
cx new <parent-id>      create a child node under parent
cx edit <id>            open body in $EDITOR
cx surface <id>         promote latent → ready
cx claim <id>           set state=claimed, part=$CX_PART (or --as <part>)
cx unclaim <id>         release claim, return to ready
cx shadow <id>          flag as shadowed
cx unshadow <id>        clear shadow flag
cx integrate <id>       mark done, move to archive
cx block <a> <b>        add blocks edge: a blocks b
cx unblock <a> <b>      remove blocks edge
cx relate <a> <b>       add related edge
cx discover <a> <b>     add discovered-from edge (a discovered-from b)
```

### Ready work computation

```sql
SELECT n.id, n.title, n.part
FROM nodes n
WHERE n.state = 'ready'
  AND n.shadowed = 0
  AND NOT EXISTS (
    SELECT 1 FROM edges e
    JOIN nodes blocker ON e.from_id = blocker.id
    WHERE e.to_id = n.id
      AND e.type = 'blocks'
      AND blocker.state != 'integrated'
  )
```

---

## Git Integration

`complex` is designed to live inside the project repository:

```
my-project/
  .complex/       ← tracked by git
  src/
  ...
```

**What git sees:**
- `graph.json` diffs show state changes, claims, edge additions
- `issues/*.md` diffs show content changes per node
- `archive/` accumulates completed work — `git log -- .complex/archive/` is a completion history

**No hooks required.** Agents commit `.complex/` changes alongside their
code changes. A claim commit looks like:

```
claim(a3F2.bX7c): implement JWT tokens

Co-authored-by: agent-backend
```

Merge conflicts are resolved at the JSON/markdown layer — structured and
human-readable. Two agents claiming different nodes produce non-conflicting
diffs by design (different keys in `graph.json`).

---

## Runtime Model

On every invocation, `cx`:

1. Reads `.complex/graph.json` + all `issues/*.md` files
2. Loads into an in-memory SQLite database
3. Executes the query or mutation
4. For mutations: writes back to `graph.json` and relevant `.md` files
5. Exits

No daemon. No persistent process. No port. Startup time target: < 50ms
for projects with up to 1000 active nodes.

---

## `cx therapy` — review mode

Surfaces nodes that need human attention:

- Claimed nodes not updated in > 24h (stale)
- Shadowed nodes (blocked, avoided)
- Nodes whose parent is integrated but they are not (orphaned)
- Root complexes with no `ready` or `claimed` children (stalled)

Output is a prioritised list for human review. Intended to be run
periodically or before a planning session.

---

## Environment Variables

```
CX_PART       identity of the current agent/user (used by cx claim)
CX_DIR        override .complex/ directory location
EDITOR        used by cx edit
```

---

## What We Are Not Building

Deliberately excluded (cf. beads):

- Network sync / federation / Dolt
- Messaging / inter-agent mail
- Molecule / proto / wisp workflow phases
- HOP (entity tracking, CV chains, attestation)
- Gates + slots (async coordination primitives)
- Compaction / semantic memory decay
- Multi-role access control
- MCP server (can be added later as a thin wrapper)
- Web UI

---

## Implementation

- Language: **Rust**
- Storage: JSON files (source of truth) + in-memory SQLite (query engine)
- ID generation: 4-char base62 random segments, dot-separated for hierarchy
- SQLite driver: `rusqlite`
- JSON: `serde_json`
- CLI: `clap`
- Target: single static binary, `cx`

---

## Success Criteria

1. `cx surface --json` returns actionable work in < 50ms on a cold start
2. Two agents can claim different nodes and `git merge` without conflict
3. `cx therapy` surfaces genuinely stuck work without false positives
4. A human can `cx add` a new complex and decompose it into tasks in < 2 minutes
5. `git log -- .complex/` tells the story of what happened in a session
