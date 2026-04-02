## Problem

`graph.json` is a monolithic JSON file. When multiple git branches make
independent cx mutations (different nodes), merging both branches to main
produces text-level merge conflicts even though the changes are semantically
independent.

This is a blocker for workflows that run parallel tasks on separate branches.
Each branch only touches its own nodes, but git sees two edits to the same
file and reports CONFLICT.

## Observed in

Ox workflow engine running two code-task pipelines in parallel (3BAe and 8ndH).
Both branches modified `graph.json` (each integrating their own task). 8ndH
merged first. 3BAe's merge then failed with conflicts in `graph.json` and
`events.jsonl`. The 3BAe node was lost from the graph entirely.

## Impact

- Parallel task branches can't merge cleanly
- Nodes silently disappear from the graph when merge conflicts are
  resolved by picking one side
- `events.jsonl` has the same problem (append-only, but both branches
  append different lines at the same position)

## Possible approaches

1. **One file per node** — split graph.json into per-node files
   (e.g. `.complex/nodes/{id}.json`). Each branch only creates/modifies
   its own node file. Merges are always clean because different files
   are touched. Index file rebuilt on read.

2. **Custom merge driver** — register a git merge driver for graph.json
   that does structural JSON merge (union of keys). Works but requires
   `.gitattributes` setup and a merge driver binary.

3. **CRDT-style append log** — replace graph.json with an append-only
   operation log. Reconstruct state by replaying. Similar to events.jsonl
   but authoritative. Merges are always clean (append + append = both appended).

4. **events.jsonl as source of truth** — events.jsonl already logs every
   mutation. If the graph could be reconstructed from the event log,
   graph.json becomes a cache that doesn't need to be committed. Only
   events.jsonl is committed, and append-only files merge cleanly
   (with minor ordering issues).

Option 1 is simplest and most git-native. Option 4 is most elegant but
a bigger change.
