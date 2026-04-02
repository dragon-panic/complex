## Append-only comment thread on cx issues

### Problem

Agents overwrite each other's work in the issue body. The spec gets
clobbered by proposals, proposals by reviews. Prompt engineering
("please preserve existing sections") is unreliable at scale.

### Design

Each issue has an ordered list of comments. The original body is the
first entry. Everything after is append-only by default — but authors
can edit or remove their own comments.

```rust
struct Comment {
    timestamp: DateTime<Utc>, // ID + ordering — ISO 8601
    author: String,           // who wrote it
    tag: Option<String>,      // what kind: proposal, review, retro, etc.
    body: String,             // markdown content
}
```

Timestamp is the comment ID. ISO 8601 format (e.g. `2026-03-29T08:15:32Z`).
Monotonically increasing (ignoring clock skew). Easy for agents to reason
about, UIs to localize, and tools to sort.

Tags are just labels for filtering — not a fixed enum. Conventions:
- `proposal` — implementation plan
- `review` — plan review verdict + rationale
- `code-review` — code review verdict + rationale
- `retro` — retrospective / lessons learned
- No tag — general discussion

Multiple comments can share the same tag (e.g. two `code-review`
entries after a fail→implement→review cycle).

### CLI

```
# Append a comment
cx comment {id} --tag proposal --file /tmp/plan.md
cx comment {id} --tag code-review "PASS — looks good"
cx comment {id} "general note, no tag"

# Read comments
cx comments {id}                    # full thread in order
cx comments {id} --tag proposal     # filter by tag
cx show {id}                        # body + thread

# Edit a comment (by ISO timestamp)
cx comment {id} --edit 2026-03-29T08:15:32Z --file /tmp/updated.md
cx comment {id} --edit 2026-03-29T08:15:32Z "updated text"

# Remove a comment
cx comment {id} --rm 2026-03-29T08:15:32Z

# Who/when is automatic
cx comment {id} --as worker-qZgk-0 --tag proposal --file /tmp/plan.md
# Falls back to CX_FILED_BY or git user if --as not given
```

### Storage

Add `comments: Vec<Comment>` to the Node struct. Serialized in
graph.json alongside the existing fields. The original `body` field
stays as-is for backward compat — it's the spec, immutable after
creation. Comments are a separate ordered list.

### Workflow integration

code-task steps use comments instead of body edits:
- propose: `cx comment {task} --tag proposal --file /tmp/plan.md`
- review-plan: `cx comment {task} --tag review --file /tmp/verdict.md`
- implement: reads `cx comments {task} --tag proposal` for the plan
- review-code: `cx comment {task} --tag code-review --file /tmp/verdict.md`

Each step appends. The full history of decisions is preserved in order.

### Why not metadata keys?

Metadata (key→value map) loses ordering and doesn't handle multiple
entries per type (two code reviews after a retry). A comment thread
is the natural model for a conversation between agents about a task.
