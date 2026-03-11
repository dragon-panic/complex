# Tag Propagation

## Design

- Add `tags: Vec<String>` field to `Node` struct
- Tags are stored only where explicitly set (own tags)
- Effective tags = own tags ∪ ancestors' tags, computed at read time by walking the hierarchical ID
- On `cx integrate`, snapshot effective tags into the archived record (denormalize on archive)

## Key behaviors

- **Inheritance**: child inherits all ancestor tags automatically
- **Override**: if a child sets the same tag, it simply appears in both own and inherited — union semantics
- **Reparent (`cx move`)**: no data migration needed, inheritance recomputes from new position
- **Archive**: effective tags baked in at integrate time since parent tree may not exist later

## CLI surface

- `cx tag <id> <tag>` — add own tag
- `cx untag <id> <tag>` — remove own tag
- `cx tags [id]` — show effective tags for a node (or list all tags in use)
- `--tag <tag>` filter on `cx list`, `cx find`, `cx tree`
- `--tag <tag>` on `cx add` / `cx new` to set at creation

## Implementation plan

1. Add `tags` field to Node model + serde + migration
2. Add `cx tag` / `cx untag` commands
3. Add effective_tags() computation (walk ancestors)
4. Materialize effective tags in SQLite during db::materialize()
5. Add --tag filter to list/find/tree
6. Denormalize effective tags on archive (integrate)
7. Add --tag to add/new for convenience
8. Tests
