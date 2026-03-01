# Orchestration, Workflows, Agents, Issues — Design Notes

## The four concepts

These sit on two axes:

```
              DATA            EXECUTION
COORDINATION  Issues          Orchestration
ACTION        Workflows       Agents
```

### Issues (bottom-left — what complex does now)

Pure data. What needs doing, current state, relationships, history.
`complex` is and should remain a **passive registry** — it stores state
and answers queries. It does not drive anything.

Issues have depth (latent → ready → claimed → integrated), a shadow flag,
dependencies (blocks edges), hierarchy (encoded in IDs), and metadata
(arbitrary JSON for external systems to read/write).

### Agents (bottom-right — executors)

Entities that claim issues, do work, and integrate. Currently just a string
label (`CX_PART`). Complex records what they've claimed via `agents.json`
(upserted on every `cx claim`) and the events log.

An agent does not live inside complex. It acts *on* complex.

The minimal agent loop (from AGENT.md):
1. `cx surface --json` — find available work
2. `cx claim <id> --as <name>` — take ownership
3. do the work
4. `cx integrate <id>` — done, unblocks dependents
5. `git add .complex/ && git commit`

### Orchestration (top-right — the missing layer)

Currently entirely absent. An orchestrator:
- Decides which agent should pick up which ready issue
- Monitors progress, intervenes when things stall (`cx therapy`)
- Handles failures, retries, escalation
- Matches work to agent capability (via `meta` field on nodes)
- Watches `events.jsonl` for state changes without polling

`cx therapy` is embryonic orchestration — it surfaces issues needing
intervention — but it doesn't act.

An orchestrator is a **separate tool** that speaks to complex via its
JSON API (`--json` flag on all commands). It could be:
- A human reviewing `cx therapy` periodically
- A simple shell loop: `cx surface --json | pick_agent | cx claim`
- A full LLM-based orchestrator reasoning about agent capabilities

### Workflows (top-left — the interesting middle ground)

Currently there is exactly one hardcoded workflow:
`latent → ready → claimed → integrated`

But real work has richer patterns:
- Some tasks need human review before integration
- Some need two agents to converge (fanout/fanin)
- Some auto-surface when their parent is claimed
- Some have retry semantics on failure

A **workflow** is a state machine spec governing how a class of issue
moves through its lifecycle. Right now that machine is hardcoded in complex.

**Design decision:** complex should NOT enforce workflows. It provides the
state primitives. An orchestrator implements workflow by:
1. Reading `meta.workflow` on a node (e.g. `"needs-review"`)
2. Acting accordingly (creating a review sub-task as a dependency, etc.)
3. Only surfacing/claiming the original when the dependency integrates

This preserves the passive registry model while giving orchestrators
something to act on.

---

## What complex should add to enable this model

These three additions (now shipped in v0.1.0) make complex composable
without overstepping into orchestration:

1. **Node metadata** (`cx meta <id> <json>`) — orchestrators and workflow
   engines write hints here. Complex ignores it. Examples:
   - `{"workflow": "needs-review"}`
   - `{"capability": "rust", "priority": 2}`
   - `{"retry_count": 1, "last_error": "tests failed"}`

2. **Events log** (`events.jsonl`, rotating) — append-only record of every
   mutation. An orchestrator can `tail -f` or process on demand. Enables
   reactive orchestration without polling graph.json.

3. **Agent registry** (`agents.json`, bounded) — upserted on every claim.
   Lets an orchestrator know who is active and when they were last seen.

---

## What does NOT belong in complex

- Actually assigning work to agents — orchestrator decision
- Running agents — orchestrator
- Failure/retry handling — orchestrator
- Approval gates — a dependency convention + orchestrator
- Agent capability matching — orchestrator reads `meta.capability`
- Workflow enforcement — orchestrator reads `meta.workflow`

---

## The composability picture

```
┌─────────────────────────────────────────┐
│           orchestrator (ox?)            │
│  - reads cx surface/therapy/parts --json│
│  - watches events.jsonl                 │
│  - assigns agents to issues             │
│  - implements workflow rules via meta   │
│  - handles retries/escalation           │
└──────────────┬──────────────────────────┘
               │  cx commands (--json)
┌──────────────▼──────────────────────────┐
│         complex (cx)                    │
│  passive registry                       │
│  issues · state · deps · meta · events  │
└──────────────┬──────────────────────────┘
               │  cx claim/integrate
┌──────────────▼──────────────────────────┐
│         agents                          │
│  claim · work · integrate               │
│  read AGENT.md / cx agent               │
└─────────────────────────────────────────┘
```

The orchestrator and agents are both clients of complex.
Neither lives inside it.

---

## Next: the orchestrator (ox?)

A minimal orchestrator for multi-agent Claude Code workflows might:

1. Read `cx surface --json` to find available work
2. Read `cx agents --json` to see active agents and their load
3. Assign: `cx claim <id> --as <agent>` on their behalf, or signal them
4. Watch `events.jsonl` for claims, integrations, stalls
5. Run `cx therapy --json` on a schedule, escalate stale items
6. Interpret `meta.workflow` to implement review gates, fanout, retries

The interface between orchestrator and complex is entirely the `cx` CLI
with `--json`. No new APIs needed in complex.

**Open questions for the orchestrator design:**
- How does an orchestrator signal an agent? (message file? env var? separate channel?)
- Does the orchestrator itself get a `CX_PART` identity in complex?
- Should `cx meta` support merge-patch (update a key) vs full replace?
- Should there be a `cx watch` command that streams events.jsonl in real time?
- Should agent capability be declared in `agents.json` or always in node `meta`?
