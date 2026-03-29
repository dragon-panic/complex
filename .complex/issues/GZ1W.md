## Problem

Archiving drops edges permanently. If both endpoints of an edge get archived,
the edge is lost entirely — neither node's archive entry contains it.
There is also no `unarchive` command to restore nodes.

## Design

### Archived edges pool (`archive/edges.jsonl`)

When archiving node X:
1. Collect all edges where `from == X` or `to == X` from the live graph.
2. Append them to `archive/edges.jsonl`.
3. Remove them from `graph.edges`.

When unarchiving node X:
1. Find node X in archive JSONL files, remove that line.
2. Move `archive/{X}.md` → `issues/{X}.md` and same for `.comments.json`.
3. Insert node back into `graph.nodes` (state remains integrated).
4. Scan `archive/edges.jsonl`: restore any edge where BOTH endpoints now exist
   in the live graph. Remove restored edges from the file.

### `cx rm` cleanup

When removing a node via `cx rm`, also scrub `archive/edges.jsonl` to remove
any edge referencing the deleted ID.

### CLI

```
cx unarchive <id>     # restore archived node (set to integrated)
    --reason          # optional rationale
```

## Subtasks

1. `store::archive_edges` — move edges to `edges.jsonl` on archive
2. `store::unarchive_node` — reverse of archive_node
3. CLI `Unarchive` subcommand
4. Scrub archived edges on `cx rm`
5. Integration tests for all 6 scenarios from design discussion
