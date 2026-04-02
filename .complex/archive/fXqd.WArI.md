Replace graph.json with nodes/{id}.json files:
- model.rs: add OutgoingEdge struct, outgoing_edges field on Node (serde as "edges")
- store.rs load(): glob nodes/*.json, deserialize, expand edges. Migration from graph.json.
- store.rs save(): collect outgoing edges per node, write nodes/{id}.json. Delete stale files.
- store.rs init(): create nodes/ dir
- store.rs find_root(): detect by nodes/ or graph.json
- Filter dormant edges (target not in graph) on load
- Update test helpers
