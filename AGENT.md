# complex — agent guide

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
cx surface --json                 ready tasks (no open blockers)
cx claim <id> --as <name>         take ownership (or set CX_PART env var)
cx unclaim <id>                   release if you cannot complete it
cx integrate <id>                 mark done → archive, unblocks dependents
cx new <parent-id> <title>        create a child task under a parent
cx discover <new-id> <source-id>  record task found while working on source
cx shadow <id>                    flag as blocked/stuck
cx show <id> --json               full node detail: state, edges, body, children
cx tree --json                    full hierarchy with states
cx parts --json                   what each part currently holds
cx therapy --json                 stale (claimed >24h) and shadowed nodes
cx list --state claimed --json    all nodes in a given state
```

## State model

```
latent → ready → claimed → integrated
                    ↕
                 shadowed  (flag — orthogonal to state)
```

## IDs

Hierarchy is encoded in the ID:
  a3F2              root complex
  a3F2.bX7c         child task
  a3F2.bX7c.Qd4e   grandchild subtask

Short IDs (leaf segment) work when unambiguous:  cx claim bX7c

## Environment

  CX_PART   your identity — set this before claiming anything

## What to commit

After any cx mutation, stage and commit `.complex/`:
  git add .complex/ && git commit -m "claim(bX7c): implement JWT tokens"
  git add .complex/ && git commit -m "integrate(bX7c): implement JWT tokens"
