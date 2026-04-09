# Design

## The name

Named after the Jungian *complex* — a cluster of charged, associated material
that pulls behaviour toward it. That's what a project really is.

| Jungian term  | cx meaning                               |
|---------------|------------------------------------------|
| Complex       | A root-level cluster of related work     |
| Part          | An agent (or agent role) claiming work   |
| Shadow        | A node that is blocked or being avoided  |
| Individuation | Completion — a node is *integrated*      |
| Therapy       | Reviewing stale, stuck, or shadowed work |

## Passive registry

Complex is a **passive registry**. It stores state and answers queries.
It does not drive anything — no scheduling, no assignment, no retries.

An orchestrator (like [ox](https://github.com/dragon-panic/ox)) implements
those behaviours by reading and writing complex via the `cx` CLI with `--json`.
Agents are also clients of complex — they claim work, do it, and integrate.

```
┌─────────────────────────────────────┐
│         orchestrator (ox)           │
│  assigns agents · watches events   │
│  implements workflows · retries    │
└──────────────┬──────────────────────┘
               │  cx commands (--json)
┌──────────────▼──────────────────────┐
│         complex (cx)                │
│  passive registry                   │
│  nodes · state · deps · meta       │
│  events · tags · comments          │
└──────────────┬──────────────────────┘
               │  cx claim/integrate
┌──────────────▼──────────────────────┐
│         agents                      │
│  claim · work · integrate           │
└─────────────────────────────────────┘
```

## Edge types

Edges fall into two categories:

**Workflow edges** affect `cx surface` (what work is available):

- `blocks` — target cannot become ready until source integrates
- `waits-for` — target waits for all children of source to integrate (fanout gate)

**Association edges** are informational only:

- `discovered-from` — this node was found while working on the source
- `related` — loose see-also link

## Storage model

Source of truth is JSON files on disk (human-readable, git-diffable).
On every invocation, `cx` materializes the graph into an **in-memory SQLite**
database for query power (ready-node computation, filtering, etc.), then
writes mutations back to disk.

No daemon, no persistent process, no port.

### Change history

`cx log` reads from `git log` — committed changes to `.complex/` are the
event stream. No separate event files; forces all changes to be committed
to be visible.

### Archive

When nodes are integrated and then archived, their node file moves from
`nodes/{id}.json` to `archive/nodes/{id}.json`. Outgoing edges travel
with the node. Incoming edges from live nodes stay dormant in those nodes'
files (filtered on load, auto-reconnect on unarchive).

## Metadata

`cx meta <id> <json>` writes an arbitrary JSON blob onto a node. Complex
ignores it entirely — it exists for orchestrators and external tools to
store workflow hints, priorities, capabilities, retry counts, etc.

## What complex does not do

These are orchestrator concerns, deliberately excluded:

- Assigning work to agents
- Running agents
- Failure/retry handling
- Approval gates (implement as dependencies + orchestrator logic)
- Agent capability matching (orchestrator reads `meta`)
- Workflow enforcement (orchestrator reads `meta`)
- Agent liveness tracking

## Comparisons

**vs markdown task lists** — not queryable, no dependency awareness, two
agents editing the same file produce merge conflicts, no machine-readable
way to ask "what can I work on right now?"

**vs hosted issue trackers** (GitHub Issues, Linear, Jira) — no network
dependency, no API keys, no webhooks. Lives in the repo, ships with the
code, travels with the clone.

**vs [beads](https://github.com/steveyegge/beads)** — beads was a direct
inspiration. Complex is the 20% that covers 80% of agent workflows:
hierarchy, state, blocking dependencies, parallel-by-default, and
git-native storage. No Dolt, no federation, no molecules, no mail system.
