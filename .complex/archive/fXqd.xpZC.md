Adapt archive/unarchive for per-node files:
- archive_node(): move nodes/{id}.json to archive/nodes/{id}.json. Don't touch other nodes' edges.
- unarchive_node(): move file back. Dormant edges auto-reconnect.
- scrub for cx rm: scan all node files, remove edges to deleted node.
- load_archived_ids(): scan archive/nodes/*.json
- Remove archive/edges.jsonl mechanism
- Migration from archive.jsonl to per-node archive files
