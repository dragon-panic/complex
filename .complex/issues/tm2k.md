## Plan

Option 2: collision detection on create, retry on hit.

### Changes

1. **`src/store.rs`** — add `load_archived_ids(root) -> HashSet<String>` that
   reads `archive/archive.jsonl` + all rotated `archive/*.jsonl` files,
   extracts the `"id"` field from each line.

2. **`src/id.rs`** — change `generate` to accept `existing: &HashSet<String>`,
   loop up to 10 attempts, bail if all collide.

3. **`src/main.rs`** — in `cmd_add` and `cmd_new`, build the existing-ID set
   from live graph + archived IDs, pass to `generate`.

4. **`tests/integration.rs`** — test that creating many nodes never produces
   a duplicate (including after archiving).

### Cost

~1-13 extra file reads per create (active archive + rotated monthly files).
Optimize later if needed.

### Acceptance criteria

- `cx add` / `cx new` never produce a duplicate ID
- Collision detected → regenerate (bounded retry, fail loudly if stuck)
- Archived/integrated nodes included in collision check
