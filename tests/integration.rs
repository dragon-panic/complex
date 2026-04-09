use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::TempDir;

// ── helpers ───────────────────────────────────────────────────────────────────

fn cx(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("cx").unwrap();
    cmd.current_dir(dir.path());
    cmd
}

fn init(dir: &TempDir) {
    cx(dir).arg("init").assert().success();
}

/// Add a root complex, return its id.
fn add(dir: &TempDir, title: &str) -> String {
    let out = cx(dir)
        .args(["--json", "add", title])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["id"].as_str().unwrap().to_string()
}

/// Add a child node, return its id.
fn new_child(dir: &TempDir, parent: &str, title: &str) -> String {
    let out = cx(dir)
        .args(["--json", "new", parent, title])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["id"].as_str().unwrap().to_string()
}

fn surface_id(dir: &TempDir, id: &str) {
    cx(dir).args(["surface", id]).assert().success();
}

fn claim(dir: &TempDir, id: &str, part: &str) {
    cx(dir).args(["claim", id, "--as", part]).assert().success();
}

fn integrate(dir: &TempDir, id: &str) {
    cx(dir).args(["integrate", id]).assert().success();
}

fn archive(dir: &TempDir, id: &str) {
    cx(dir).args(["archive", "--ids", id]).assert().success();
}


/// Reconstruct a graph-json-like structure from per-node files.
/// Filters dormant edges (where target is not a live node), matching runtime behavior.
fn graph_json(dir: &TempDir) -> serde_json::Value {
    let nodes_dir = dir.path().join(".complex/nodes");
    let mut nodes = Vec::new();
    let mut all_edges = Vec::new();
    let mut live_ids = std::collections::HashSet::new();
    if nodes_dir.exists() {
        for entry in std::fs::read_dir(&nodes_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let raw = std::fs::read_to_string(&path).unwrap();
                let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
                let node_id = v["id"].as_str().unwrap().to_string();
                live_ids.insert(node_id.clone());
                if let Some(node_edges) = v["edges"].as_array() {
                    for e in node_edges {
                        all_edges.push(serde_json::json!({
                            "from": node_id,
                            "to": e["to"],
                            "type": e["type"],
                        }));
                    }
                }
                nodes.push(v);
            }
        }
    }
    // Filter dormant edges (target not in live graph)
    let edges: Vec<_> = all_edges.into_iter()
        .filter(|e| live_ids.contains(e["to"].as_str().unwrap()))
        .collect();
    serde_json::json!({ "version": 1, "nodes": nodes, "edges": edges })
}

fn archive_entries(dir: &TempDir) -> Vec<serde_json::Value> {
    let archive_nodes = dir.path().join(".complex/archive/nodes");
    let mut entries = Vec::new();
    if archive_nodes.exists() {
        for entry in std::fs::read_dir(&archive_nodes).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let raw = std::fs::read_to_string(&path).unwrap();
                entries.push(serde_json::from_str(&raw).unwrap());
            }
        }
    }
    // Also check legacy JSONL
    let legacy = dir.path().join(".complex/archive/archive.jsonl");
    if legacy.exists() {
        let raw = std::fs::read_to_string(legacy).unwrap();
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            entries.push(serde_json::from_str(line).unwrap());
        }
    }
    entries
}

fn unarchive(dir: &TempDir, id: &str) {
    cx(dir).args(["unarchive", id]).assert().success();
}

fn block(dir: &TempDir, a: &str, b: &str) {
    cx(dir).args(["block", a, b]).assert().success();
}

fn graph_edges(dir: &TempDir) -> Vec<serde_json::Value> {
    let g = graph_json(dir);
    g["edges"].as_array().unwrap().clone()
}

fn graph_node_ids(dir: &TempDir) -> Vec<String> {
    let g = graph_json(dir);
    g["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect()
}

/// Collect all non-live edges: outgoing edges from archived nodes +
/// dormant edges from live nodes pointing to non-live targets.
fn archived_edges(dir: &TempDir) -> Vec<serde_json::Value> {
    let mut edges = Vec::new();

    // Collect live node IDs
    let nodes_dir = dir.path().join(".complex/nodes");
    let mut live_ids = std::collections::HashSet::new();
    if nodes_dir.exists() {
        for entry in std::fs::read_dir(&nodes_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let raw = std::fs::read_to_string(&path).unwrap();
                let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
                live_ids.insert(v["id"].as_str().unwrap().to_string());
            }
        }
    }

    // Edges from archived node files
    let archive_nodes = dir.path().join(".complex/archive/nodes");
    if archive_nodes.exists() {
        for entry in std::fs::read_dir(&archive_nodes).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let raw = std::fs::read_to_string(&path).unwrap();
                let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
                let from = v["id"].as_str().unwrap().to_string();
                if let Some(node_edges) = v["edges"].as_array() {
                    for e in node_edges {
                        edges.push(serde_json::json!({
                            "from": from,
                            "to": e["to"],
                            "type": e["type"],
                        }));
                    }
                }
            }
        }
    }

    // Dormant edges from live nodes (pointing to non-live targets)
    if nodes_dir.exists() {
        for entry in std::fs::read_dir(&nodes_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let raw = std::fs::read_to_string(&path).unwrap();
                let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
                let from = v["id"].as_str().unwrap().to_string();
                if let Some(node_edges) = v["edges"].as_array() {
                    for e in node_edges {
                        let to = e["to"].as_str().unwrap();
                        if !live_ids.contains(to) {
                            edges.push(serde_json::json!({
                                "from": from,
                                "to": to,
                                "type": e["type"],
                            }));
                        }
                    }
                }
            }
        }
    }

    edges
}

fn has_edge(edges: &[serde_json::Value], from: &str, to: &str) -> bool {
    edges.iter().any(|e| e["from"].as_str() == Some(from) && e["to"].as_str() == Some(to))
}

// ── cx status ─────────────────────────────────────────────────────────────────

#[test]
fn status_shows_tree_and_surface() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "My Project");
    let child = new_child(&dir, &root, "Task A");
    surface_id(&dir, &child);

    cx(&dir).args(["status"]).assert().success()
        .stdout(contains("My Project"))
        .stdout(contains("Task A"))
        .stdout(contains("ready"));
}

#[test]
fn status_json_has_tree_and_ready() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Root");
    let child = new_child(&dir, &root, "Child");
    surface_id(&dir, &child);

    let out = cx(&dir).args(["--json", "status"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["tree"].is_array());
    assert!(v["ready"].is_array());
    assert_eq!(v["tree"][0]["title"], "Root");
    assert_eq!(v["ready"][0]["title"], "Child");
}

#[test]
fn status_empty_project() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    cx(&dir).args(["status"]).assert().success();
}

// ── cx init ───────────────────────────────────────────────────────────────────

#[test]
fn init_creates_structure() {
    let dir = TempDir::new().unwrap();
    cx(&dir).arg("init").assert().success();

    let root = dir.path().join(".complex");
    assert!(root.exists());
    assert!(root.join("nodes").is_dir());
    assert!(root.join("issues").is_dir());
    assert!(root.join("archive").is_dir());

    // Empty project should have no node files
    let g = graph_json(&dir);
    assert!(g["nodes"].as_array().unwrap().is_empty());
    assert!(g["edges"].as_array().unwrap().is_empty());
}

#[test]
fn init_twice_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    cx(&dir).arg("init").assert().failure();
}

#[test]
fn commands_outside_project_fail() {
    let dir = TempDir::new().unwrap(); // no init
    cx(&dir).args(["add", "anything"]).assert().failure()
        .stderr(contains("cx init"));
}

// ── CX_DIR env var ───────────────────────────────────────────────────────────

#[test]
fn cx_dir_overrides_root() {
    let dir = TempDir::new().unwrap();
    let custom = dir.path().join("custom-cx");

    // init with CX_DIR
    cx(&dir).arg("init")
        .env("CX_DIR", &custom)
        .assert().success();
    assert!(custom.join("nodes").is_dir());
    assert!(!dir.path().join(".complex").exists());

    // add with CX_DIR
    let out = cx(&dir).args(["--json", "add", "Test task"])
        .env("CX_DIR", &custom)
        .output().unwrap();
    assert!(out.status.success());

    // status with CX_DIR
    cx(&dir).arg("status")
        .env("CX_DIR", &custom)
        .assert().success().stdout(contains("Test task"));
}

#[test]
fn cx_dir_missing_graph_errors() {
    let dir = TempDir::new().unwrap();
    let bad = dir.path().join("nonexistent");

    cx(&dir).args(["add", "anything"])
        .env("CX_DIR", &bad)
        .assert().failure()
        .stderr(contains("CX_DIR"));
}

#[test]
fn cx_dir_takes_priority_over_local() {
    let dir = TempDir::new().unwrap();
    let custom = dir.path().join("custom-cx");

    // init both: local .complex/ and custom CX_DIR
    init(&dir);
    cx(&dir).arg("init")
        .env("CX_DIR", &custom)
        .assert().success();

    // add to local
    let local_id = add(&dir, "Local task");

    // add to custom
    let out = cx(&dir).args(["--json", "add", "Custom task"])
        .env("CX_DIR", &custom)
        .output().unwrap();
    assert!(out.status.success());

    // status with CX_DIR should show custom, not local
    cx(&dir).arg("status")
        .env("CX_DIR", &custom)
        .assert().success()
        .stdout(contains("Custom task"));

    // status without CX_DIR should show local
    cx(&dir).arg("status")
        .assert().success()
        .stdout(contains("Local task"));

    // confirm they don't leak into each other
    let custom_status = cx(&dir).arg("status")
        .env("CX_DIR", &custom)
        .output().unwrap();
    assert!(!String::from_utf8_lossy(&custom_status.stdout).contains(&local_id));
}

#[test]
fn cx_dir_existing_dir_no_graph_errors() {
    let dir = TempDir::new().unwrap();
    let partial = dir.path().join("partial-cx");
    std::fs::create_dir_all(&partial).unwrap();

    cx(&dir).args(["add", "anything"])
        .env("CX_DIR", &partial)
        .assert().failure()
        .stderr(contains("CX_DIR"));
}

// ── --ephemeral ──────────────────────────────────────────────────────────────

#[test]
fn init_ephemeral_adds_gitignore() {
    let dir = TempDir::new().unwrap();
    cx(&dir).args(["init", "--ephemeral"])
        .assert().success()
        .stdout(contains(".gitignore"));

    let gi = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(gi.contains(".complex/"));
}

#[test]
fn init_ephemeral_appends_to_existing_gitignore() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();

    cx(&dir).args(["init", "--ephemeral"])
        .assert().success();

    let gi = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(gi.contains("target/"));
    assert!(gi.contains(".complex/"));
}

#[test]
fn init_ephemeral_no_duplicate_in_gitignore() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(".gitignore"), ".complex/\n").unwrap();

    cx(&dir).args(["init", "--ephemeral"])
        .assert().success();

    let gi = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert_eq!(gi.matches(".complex/").count(), 1);
}

#[test]
fn init_without_ephemeral_no_gitignore() {
    let dir = TempDir::new().unwrap();
    cx(&dir).arg("init").assert().success();
    assert!(!dir.path().join(".gitignore").exists());
}

#[test]
fn init_ephemeral_with_external_cx_dir_skips_gitignore() {
    let dir = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    let cx_path = external.path().join("ext-cx");

    cx(&dir).args(["init", "--ephemeral"])
        .env("CX_DIR", &cx_path)
        .assert().success()
        .stdout(contains("--ephemeral ignored"));

    // no .gitignore created since CX_DIR is outside the project
    assert!(!dir.path().join(".gitignore").exists());
}

// ── cx add ────────────────────────────────────────────────────────────────────

#[test]
fn add_creates_root_node() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    cx(&dir).args(["add", "My Complex"])
        .assert().success()
        .stdout(contains("My Complex"))
        .stdout(contains("created"));

    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["title"], "My Complex");
    assert_eq!(nodes[0]["state"], "latent");
    // root id has no dots
    let id = nodes[0]["id"].as_str().unwrap();
    assert!(!id.contains('.'));
    assert_eq!(id.len(), 4);
}

#[test]
fn add_json_output() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    let out = cx(&dir).args(["--json", "add", "Test"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["title"], "Test");
    assert!(v["id"].as_str().is_some());
}

#[test]
fn add_multi_word_title() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "User Authentication Flow");
    assert_eq!(id.len(), 4);
    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["title"], "User Authentication Flow");
}

// ── cx new ────────────────────────────────────────────────────────────────────

#[test]
fn new_creates_child_with_flat_id_and_parent_field() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let parent = add(&dir, "Auth");
    let child = new_child(&dir, &parent, "Implement JWT");

    // Flat 4-char ID, no dots
    assert_eq!(child.len(), 4);
    assert!(!child.contains('.'));

    // parent field set correctly in graph
    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    let child_node = nodes.iter().find(|n| n["id"].as_str().unwrap() == child).unwrap();
    assert_eq!(child_node["parent"].as_str().unwrap(), parent);
    assert_eq!(nodes.len(), 2);
}

#[test]
fn new_accepts_short_parent_id() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let parent = add(&dir, "Auth");
    let child = new_child(&dir, &parent, "JWT task");
    assert_eq!(child.len(), 4);

    let g = graph_json(&dir);
    let child_node = g["nodes"].as_array().unwrap()
        .iter().find(|n| n["id"].as_str().unwrap() == child).unwrap();
    assert_eq!(child_node["parent"].as_str().unwrap(), parent);
}

#[test]
fn new_grandchild_has_parent_chain() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Root");
    let child = new_child(&dir, &root, "Child");
    let grandchild = new_child(&dir, &child, "Grandchild");

    assert_eq!(grandchild.len(), 4);

    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    let gc_node = nodes.iter().find(|n| n["id"].as_str().unwrap() == grandchild).unwrap();
    assert_eq!(gc_node["parent"].as_str().unwrap(), child);
    let c_node = nodes.iter().find(|n| n["id"].as_str().unwrap() == child).unwrap();
    assert_eq!(c_node["parent"].as_str().unwrap(), root);
}

#[test]
fn new_bad_parent_fails_with_hint() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    cx(&dir).args(["new", "doesnotexist", "title"])
        .assert().failure()
        .stderr(contains("cx tree"));
}

// ── parent field ─────────────────────────────────────────────────────────────

#[test]
fn parent_field_set_on_new_child() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let parent = add(&dir, "Root");
    let child = new_child(&dir, &parent, "Child");

    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    let child_node = nodes.iter().find(|n| n["id"].as_str().unwrap() == child).unwrap();
    assert_eq!(child_node["parent"].as_str().unwrap(), parent);
}

#[test]
fn parent_field_none_for_root() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Root");

    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    let root_node = nodes.iter().find(|n| n["id"].as_str().unwrap() == root).unwrap();
    assert!(root_node.get("parent").is_none() || root_node["parent"].is_null());
}

#[test]
fn parent_field_updated_on_move() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let child = new_child(&dir, &a, "Child");

    cx(&dir).args(["move", &child, &b]).assert().success();

    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    let moved = nodes.iter().find(|n| n["id"].as_str().unwrap() == child).unwrap();
    assert_eq!(moved["parent"].as_str().unwrap(), b);
}

// ── cx surface ────────────────────────────────────────────────────────────────

#[test]
fn surface_list_empty_when_no_ready_nodes() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "Auth"); // latent by default
    cx(&dir).args(["surface"]).assert().success()
        .stdout(contains("no ready"));
}

#[test]
fn surface_id_promotes_latent_to_ready() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");
    cx(&dir).args(["surface", &id]).assert().success();

    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["state"], "ready");
}

#[test]
fn surface_list_shows_ready_nodes() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");
    surface_id(&dir, &id);

    cx(&dir).args(["surface"]).assert().success()
        .stdout(contains("Auth"));
}

#[test]
fn surface_multiple_ids() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Root");
    let c1 = new_child(&dir, &root, "Task A");
    let c2 = new_child(&dir, &root, "Task B");

    cx(&dir).args(["surface", &c1, &c2]).assert().success()
        .stdout(contains("surfaced"))
        .stdout(contains(&c1))
        .stdout(contains(&c2));

    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    for n in nodes {
        if n["id"] == c1 || n["id"] == c2 {
            assert_eq!(n["state"], "ready");
        }
    }
}

#[test]
fn surface_non_latent_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");
    surface_id(&dir, &id); // now ready
    cx(&dir).args(["surface", &id]).assert().failure()
        .stderr(contains("latent"));
}

#[test]
fn surface_json_output() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");
    surface_id(&dir, &id);

    let out = cx(&dir).args(["--json", "surface"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], id.as_str());
}

// ── cx claim / unclaim ────────────────────────────────────────────────────────

#[test]
fn claim_sets_state_and_part() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");
    surface_id(&dir, &id);
    cx(&dir).args(["claim", &id, "--as", "agent-1"])
        .assert().success();

    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["state"], "claimed");
    assert_eq!(g["nodes"][0]["part"], "agent-1");
}

#[test]
fn claim_uses_cx_part_env() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");
    surface_id(&dir, &id);

    let mut cmd = cx(&dir);
    cmd.env("CX_PART", "env-agent");
    cmd.args(["claim", &id]).assert().success();

    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["part"], "env-agent");
}

#[test]
fn claim_already_claimed_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-1");
    cx(&dir).args(["claim", &id, "--as", "agent-2"])
        .assert().failure()
        .stderr(contains("already claimed"));
}

#[test]
fn unclaim_returns_to_ready() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-1");
    cx(&dir).args(["unclaim", &id]).assert().success();

    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["state"], "ready");
    assert!(g["nodes"][0]["part"].is_null());
}

// ── cx integrate ──────────────────────────────────────────────────────────────

#[test]
fn integrate_moves_to_archive() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-1");
    integrate(&dir, &id);

    // node still in graph.json with state=integrated
    let g = graph_json(&dir);
    assert_eq!(g["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(g["nodes"][0]["state"], "integrated");

    // archive moves it out
    archive(&dir, &id);

    let g = graph_json(&dir);
    assert!(g["nodes"].as_array().unwrap().is_empty());

    let entries = archive_entries(&dir);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], id.as_str());
    assert_eq!(entries[0]["state"], "integrated");
}

#[test]
fn integrate_moves_markdown_to_archive_dir() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Auth");

    // write a body
    std::fs::write(
        dir.path().join(format!(".complex/issues/{}.md", id)),
        "some body",
    ).unwrap();

    surface_id(&dir, &id);
    integrate(&dir, &id);

    // body still in issues after integrate
    assert!(dir.path().join(format!(".complex/issues/{}.md", id)).exists());

    // archive moves it
    archive(&dir, &id);
    assert!(!dir.path().join(format!(".complex/issues/{}.md", id)).exists());
    assert!(dir.path().join(format!(".complex/archive/{}.md", id)).exists());
}

#[test]
fn integrate_cleans_up_edges() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    cx(&dir).args(["block", &a, &b]).assert().success();

    // edge exists
    let g = graph_json(&dir);
    assert_eq!(g["edges"].as_array().unwrap().len(), 1);

    // integrate keeps node but edges stay (needed for graph queries)
    integrate(&dir, &a);
    let g = graph_json(&dir);
    assert_eq!(g["edges"].as_array().unwrap().len(), 1);

    // archive removes edges
    archive(&dir, &a);
    let g = graph_json(&dir);
    assert!(g["edges"].as_array().unwrap().is_empty());
}

#[test]
fn integrate_unblocks_downstream() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    cx(&dir).args(["block", &a, &b]).assert().success();

    // B not in surface yet (blocked by A)
    cx(&dir).args(["surface"]).assert().success()
        .stdout(contains("A"))
        .stdout(contains("A")); // only A

    integrate(&dir, &a);

    // B now appears in surface
    cx(&dir).args(["surface"]).assert().success()
        .stdout(contains("B"));
}

#[test]
fn integrate_warns_on_active_children() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let parent = add(&dir, "Parent");
    let _child = new_child(&dir, &parent, "Child");
    surface_id(&dir, &parent);

    // Should succeed but print a warning on stderr
    cx(&dir).args(["integrate", &parent])
        .assert().success()
        .stderr(contains("warning"));
}

// ── cx block / cycle detection ────────────────────────────────────────────────

#[test]
fn block_adds_edge() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    cx(&dir).args(["block", &a, &b]).assert().success();

    let g = graph_json(&dir);
    let edges = g["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["from"], a.as_str());
    assert_eq!(edges[0]["to"], b.as_str());
    assert_eq!(edges[0]["type"], "blocks");
}

#[test]
fn block_cycle_direct_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    cx(&dir).args(["block", &a, &b]).assert().success();
    cx(&dir).args(["block", &b, &a]).assert().failure()
        .stderr(contains("cycle"));
}

#[test]
fn block_cycle_transitive_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let c = add(&dir, "C");
    cx(&dir).args(["block", &a, &b]).assert().success();
    cx(&dir).args(["block", &b, &c]).assert().success();
    // c → a would close a→b→c→a cycle
    cx(&dir).args(["block", &c, &a]).assert().failure()
        .stderr(contains("cycle"));
}

#[test]
fn unblock_removes_edge() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    cx(&dir).args(["block", &a, &b]).assert().success();
    cx(&dir).args(["unblock", &a, &b]).assert().success();

    let g = graph_json(&dir);
    assert!(g["edges"].as_array().unwrap().is_empty());
}

// ── cx list ───────────────────────────────────────────────────────────────────

#[test]
fn list_shows_all_nodes() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "Alpha");
    add(&dir, "Beta");

    cx(&dir).args(["list"]).assert().success()
        .stdout(contains("Alpha"))
        .stdout(contains("Beta"));
}

#[test]
fn list_state_filter() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "Alpha");
    add(&dir, "Beta");
    surface_id(&dir, &a);

    cx(&dir).args(["list", "--state", "ready"]).assert().success()
        .stdout(contains("Alpha"));

    let out = cx(&dir).args(["list", "--state", "ready"]).output().unwrap();
    assert!(!String::from_utf8(out.stdout).unwrap().contains("Beta"));
}

#[test]
fn list_json_output() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "Alpha");

    let out = cx(&dir).args(["--json", "list"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["title"], "Alpha");
}

// ── cx tree ───────────────────────────────────────────────────────────────────

#[test]
fn tree_shows_hierarchy() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Root");
    new_child(&dir, &root, "Child A");
    new_child(&dir, &root, "Child B");

    cx(&dir).args(["tree"]).assert().success()
        .stdout(contains("Root"))
        .stdout(contains("Child A"))
        .stdout(contains("Child B"));
}

#[test]
fn tree_scoped_to_id() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let r1 = add(&dir, "Complex 1");
    let r2 = add(&dir, "Complex 2");
    new_child(&dir, &r1, "Task under 1");
    new_child(&dir, &r2, "Task under 2");

    let out = cx(&dir).args(["tree", &r1]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Task under 1"));
    assert!(!stdout.contains("Task under 2"));
}

// ── cx parts ──────────────────────────────────────────────────────────────────

#[test]
fn parts_shows_claimed_nodes_by_agent() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    claim(&dir, &a, "agent-1");
    claim(&dir, &b, "agent-2");

    cx(&dir).args(["parts"]).assert().success()
        .stdout(contains("agent-1"))
        .stdout(contains("agent-2"));
}

#[test]
fn parts_empty_when_nothing_claimed() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    cx(&dir).args(["parts"]).assert().success()
        .stdout(contains("no claimed"));
}

// ── cx shadow / unshadow ──────────────────────────────────────────────────────

#[test]
fn shadow_flag_and_list() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Stuck Task");

    cx(&dir).args(["shadow", &id]).assert().success();

    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["shadowed"], true);

    cx(&dir).args(["shadow"]).assert().success()
        .stdout(contains("Stuck Task"));
}

#[test]
fn unshadow_clears_flag() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");
    cx(&dir).args(["shadow", &id]).assert().success();
    cx(&dir).args(["unshadow", &id]).assert().success();

    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["shadowed"], false);
}

#[test]
fn shadowed_node_excluded_from_surface() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");
    surface_id(&dir, &id);
    cx(&dir).args(["shadow", &id]).assert().success();

    cx(&dir).args(["surface"]).assert().success()
        .stdout(contains("no ready"));
}

// ── cx show ───────────────────────────────────────────────────────────────────

#[test]
fn show_displays_node_details() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "My Task");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-x");

    cx(&dir).args(["show", &id]).assert().success()
        .stdout(contains("My Task"))
        .stdout(contains("claimed"))
        .stdout(contains("agent-x"));
}

#[test]
fn show_json_includes_all_fields() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");

    let out = cx(&dir).args(["--json", "show", &id]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["id"], id.as_str());
    assert_eq!(v["title"], "Task");
    assert!(v["created_at"].as_str().is_some());
    assert!(v["blockers"].is_array());
    assert!(v["children"].is_array());
}

// ── short id resolution ───────────────────────────────────────────────────────

#[test]
fn short_id_resolves_unambiguously() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Root");
    let child = new_child(&dir, &root, "Child");

    // Extract just the leaf segment
    let leaf = child.split('.').last().unwrap();

    cx(&dir).args(["surface", leaf]).assert().success();

    let g = graph_json(&dir);
    let node = g["nodes"].as_array().unwrap()
        .iter()
        .find(|n| n["id"] == child.as_str())
        .unwrap();
    assert_eq!(node["state"], "ready");
}

#[test]
fn ambiguous_short_id_fails() {
    // Legacy repos with dot-separated IDs: two nodes sharing the same leaf
    let dir = TempDir::new().unwrap();
    let root = dir.path().join(".complex");
    std::fs::create_dir_all(root.join("issues")).unwrap();
    std::fs::create_dir_all(root.join("archive")).unwrap();

    let g = serde_json::json!({
        "version": 1,
        "nodes": [
            {
                "id": "AAAA.ZZZZ",
                "title": "First",
                "state": "latent",
                "shadowed": false,
                "part": null,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z"
            },
            {
                "id": "BBBB.ZZZZ",
                "title": "Second",
                "state": "latent",
                "shadowed": false,
                "part": null,
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z"
            }
        ],
        "edges": []
    });
    std::fs::write(root.join("graph.json"), serde_json::to_string_pretty(&g).unwrap()).unwrap();

    cx(&dir).args(["show", "ZZZZ"]).assert().failure()
        .stderr(contains("ambiguous"));
}

// ── cx therapy ────────────────────────────────────────────────────────────────

#[test]
fn therapy_all_clear_when_nothing_stuck() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "Fine");
    cx(&dir).args(["therapy"]).assert().success()
        .stdout(contains("all clear"));
}

#[test]
fn therapy_surfaces_shadowed_nodes() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Blocked Thing");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-1");
    cx(&dir).args(["shadow", &id]).assert().success();

    cx(&dir).args(["therapy"]).assert().success()
        .stdout(contains("Blocked Thing"))
        .stdout(contains("shadowed"));
}

#[test]
fn therapy_shows_reason_for_shadowed() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Stuck");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-1");
    cx(&dir).args(["shadow", &id, "--reason", "waiting for design review"])
        .assert().success();

    // Text output shows the reason
    cx(&dir).args(["therapy"]).assert().success()
        .stdout(contains("waiting for design review"));

    // JSON output includes _reason
    let out = cx(&dir).args(["--json", "therapy"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["_reason"], "waiting for design review");
}

// ── rename ───────────────────────────────────────────────────────────────────

#[test]
fn rename_updates_title() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Old title");

    cx(&dir).args(["rename", &id, "New", "title"])
        .assert().success().stdout(contains("renamed"));

    let out = cx(&dir).args(["--json", "show", &id]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["title"], "New title");
}

#[test]
fn rename_json_output() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Original");

    let out = cx(&dir).args(["--json", "rename", &id, "Updated"])
        .output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["title"], "Updated");
}

// ── add/new --body ───────────────────────────────────────────────────────────

#[test]
fn add_with_body_sets_body_at_creation() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = cx(&dir)
        .args(["--json", "add", "With body", "--body", "## Description\n\nSome details."])
        .output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    let show = cx(&dir).args(["--json", "show", id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(s["body"], "## Description\n\nSome details.");
}

#[test]
fn new_with_body_sets_body_at_creation() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let parent = add(&dir, "Parent");
    let out = cx(&dir)
        .args(["--json", "new", &parent, "Child task", "--body", "Child body content"])
        .output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    let show = cx(&dir).args(["--json", "show", id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(s["body"], "Child body content");
}

#[test]
fn add_with_body_stdin_reads_piped_input() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = cx(&dir)
        .args(["--json", "add", "Piped body", "--body", "-"])
        .write_stdin("Body from stdin pipe")
        .output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    let show = cx(&dir).args(["--json", "show", id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(s["body"], "Body from stdin pipe");
}

// ── add/new --body-file ──────────────────────────────────────────────────────

#[test]
fn add_with_body_file() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let body_file = dir.path().join("body.md");
    std::fs::write(&body_file, "# Spec\n\nDetails here.").unwrap();

    let out = cx(&dir)
        .args(["--json", "add", "From file", "--body-file", body_file.to_str().unwrap()])
        .output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    let show = cx(&dir).args(["--json", "show", id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(s["body"], "# Spec\n\nDetails here.");
}

#[test]
fn new_with_body_file() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let parent = add(&dir, "Parent");
    let body_file = dir.path().join("child-body.md");
    std::fs::write(&body_file, "Child body from file").unwrap();

    let out = cx(&dir)
        .args(["--json", "new", &parent, "Child", "--body-file", body_file.to_str().unwrap()])
        .output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    let show = cx(&dir).args(["--json", "show", id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(s["body"], "Child body from file");
}

#[test]
fn add_body_and_body_file_conflict() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let body_file = dir.path().join("body.md");
    std::fs::write(&body_file, "content").unwrap();

    cx(&dir).args(["add", "Conflict", "--body", "inline", "--body-file", body_file.to_str().unwrap()])
        .assert().failure();
}

// ── cx edit (non-interactive) ─────────────────────────────────────────────────

#[test]
fn edit_body_flag_sets_body() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Needs body");

    cx(&dir).args(["edit", &id, "--body", "Hello from --body flag"])
        .assert().success().stdout(contains("saved"));

    let out = cx(&dir).args(["--json", "show", &id]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["body"], "Hello from --body flag");
}

#[test]
fn edit_file_flag_reads_from_file() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "File body");

    let body_file = dir.path().join("body.md");
    std::fs::write(&body_file, "# Task\n\nBody from file.").unwrap();

    cx(&dir).args(["edit", &id, "--file", body_file.to_str().unwrap()])
        .assert().success().stdout(contains("saved"));

    let out = cx(&dir).args(["--json", "show", &id]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["body"], "# Task\n\nBody from file.");
}

#[test]
fn edit_stdin_sets_body_when_piped() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Stdin body");

    cx(&dir).args(["edit", &id])
        .write_stdin("Piped body content")
        .assert().success().stdout(contains("saved"));

    let out = cx(&dir).args(["--json", "show", &id]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["body"], "Piped body content");
}

#[test]
fn edit_body_no_changes_prints_message() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Same body");

    // Set initial body
    cx(&dir).args(["edit", &id, "--body", "Initial"])
        .assert().success();

    // Set same body again
    cx(&dir).args(["edit", &id, "--body", "Initial"])
        .assert().success().stdout(contains("no changes"));
}

#[test]
fn edit_body_and_file_conflict() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Conflict");

    cx(&dir).args(["edit", &id, "--body", "text", "--file", "/tmp/x"])
        .assert().failure();
}

// ── cx --reason flag ──────────────────────────────────────────────────────────

#[test]
fn reason_on_claim_writes_meta() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");
    surface_id(&dir, &id);
    cx(&dir).args(["claim", &id, "--as", "agent-1", "--reason", "matching capability"])
        .assert().success();

    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["meta"]["_reason"], "matching capability");
}

#[test]
fn reason_on_shadow_writes_meta() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");
    cx(&dir).args(["shadow", &id, "--reason", "tests failing"])
        .assert().success();

    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["meta"]["_reason"], "tests failing");
}

#[test]
fn reason_on_integrate_persists_in_archive() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-1");
    cx(&dir).args(["integrate", &id, "--reason", "all tests pass"])
        .assert().success();

    // reason stored on node in graph
    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["meta"]["_reason"], "all tests pass");

    // persists after archive
    archive(&dir, &id);
    let entries = archive_entries(&dir);
    assert_eq!(entries[0]["meta"]["_reason"], "all tests pass");
}

#[test]
fn reason_on_surface_writes_meta() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");
    cx(&dir).args(["surface", &id, "--reason", "dependency resolved"])
        .assert().success();

    let g = graph_json(&dir);
    assert_eq!(g["nodes"][0]["meta"]["_reason"], "dependency resolved");
}

#[test]
fn commands_without_reason_still_work() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-1");

    // No --reason anywhere — should work fine
    let g = graph_json(&dir);
    assert!(g["nodes"][0]["meta"].is_null() || g["nodes"][0].get("meta").is_none());
}

#[test]
fn show_displays_reason() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");
    cx(&dir).args(["shadow", &id, "--reason", "blocked on API"])
        .assert().success();

    cx(&dir).args(["show", &id]).assert().success()
        .stdout(contains("reason:   blocked on API"));

    // JSON includes meta with _reason
    let out = cx(&dir).args(["--json", "show", &id]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["meta"]["_reason"], "blocked on API");
}

#[test]
fn reason_preserves_existing_meta() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Task");

    // Set some user meta first
    cx(&dir).args(["meta", &id, r#"{"capability":"rust","priority":1}"#])
        .assert().success();

    // Shadow with reason — should merge, not overwrite
    cx(&dir).args(["shadow", &id, "--reason", "blocked"])
        .assert().success();

    let g = graph_json(&dir);
    let meta = &g["nodes"][0]["meta"];
    assert_eq!(meta["capability"], "rust");
    assert_eq!(meta["priority"], 1);
    assert_eq!(meta["_reason"], "blocked");
}

#[test]
fn therapy_surfaces_stale_claimed_nodes() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Stale Task");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-1");

    // Backdate updated_at to 2 days ago by editing the per-node file
    let node_path = dir.path().join(format!(".complex/nodes/{}.json", id));
    let raw = std::fs::read_to_string(&node_path).unwrap();
    let mut node: serde_json::Value = serde_json::from_str(&raw).unwrap();
    node["updated_at"] = serde_json::json!("2026-01-01T00:00:00Z");
    std::fs::write(&node_path, serde_json::to_string_pretty(&node).unwrap()).unwrap();

    cx(&dir).args(["therapy"]).assert().success()
        .stdout(contains("Stale Task"))
        .stdout(contains("stale"));
}

// ── auto-surface after integrate ──────────────────────────────────────────────

#[test]
fn integrate_auto_surfaces_unblocked_latent_node() {
    // A (latent) blocks B (latent). Surface and integrate A.
    // B should be auto-promoted to ready without manual cx surface B.
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    cx(&dir).args(["block", &a, &b]).assert().success();
    surface_id(&dir, &a);
    claim(&dir, &a, "agent-1");

    // B is latent and blocked — not surfaceable yet
    cx(&dir).args(["surface"]).assert().success()
        .stdout(predicates::str::contains("B").not());

    let out = cx(&dir)
        .args(["--json", "integrate", &a])
        .output()
        .unwrap();
    assert!(out.status.success());

    // JSON output should include newly_surfaced
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["state"], "integrated");
    let newly = v["newly_surfaced"].as_array().unwrap();
    assert_eq!(newly.len(), 1);
    assert!(newly[0].as_str().unwrap().ends_with(&b));

    // B should now appear in surface listing
    cx(&dir).args(["surface"]).assert().success()
        .stdout(contains("B"));
}

#[test]
fn integrate_does_not_auto_surface_if_other_blocker_remains() {
    // A blocks C. B blocks C. Integrating A should NOT auto-surface C (still blocked by B).
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let c = add(&dir, "C");
    cx(&dir).args(["block", &a, &c]).assert().success();
    cx(&dir).args(["block", &b, &c]).assert().success();
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    claim(&dir, &a, "agent-1");

    let out = cx(&dir)
        .args(["--json", "integrate", &a])
        .output()
        .unwrap();
    assert!(out.status.success());

    // No newly_surfaced — C still blocked by B
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.get("newly_surfaced").is_none());

    // C still not in surface (B still blocks it)
    cx(&dir).args(["surface"]).assert().success()
        .stdout(predicates::str::contains("C").not());
}

#[test]
fn integrate_auto_surface_ready_node_not_re_surfaced() {
    // A blocks B. B is already ready (manually surfaced). Integrating A should
    // not double-surface B (it's already ready, not latent).
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    cx(&dir).args(["block", &a, &b]).assert().success();
    surface_id(&dir, &a);
    surface_id(&dir, &b); // manually surfaced even though blocked
    claim(&dir, &a, "agent-1");

    let out = cx(&dir)
        .args(["--json", "integrate", &a])
        .output()
        .unwrap();
    assert!(out.status.success());

    // B was already ready, so not in newly_surfaced
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.get("newly_surfaced").is_none());
}

// ── cx rm ─────────────────────────────────────────────────────────────────

#[test]
fn rm_removes_node() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Mistake");
    cx(&dir).args(["rm", &id]).assert().success()
        .stdout(contains("removed"));
    let g = graph_json(&dir);
    assert!(g["nodes"].as_array().unwrap().is_empty());
}

#[test]
fn rm_refuses_with_active_children() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Parent");
    let _child = new_child(&dir, &root, "Child");
    cx(&dir).args(["rm", &root]).assert().failure()
        .stderr(contains("active child"));
}

#[test]
fn rm_cleans_up_edges() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    cx(&dir).args(["block", &a, &b]).assert().success();
    cx(&dir).args(["rm", &a]).assert().success();
    let g = graph_json(&dir);
    assert!(g["edges"].as_array().unwrap().is_empty());
}

#[test]
fn rm_archives_node() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Discarded");
    cx(&dir).args(["rm", &id]).assert().success();

    // node in archive
    let entries = archive_entries(&dir);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], id.as_str());
    assert_eq!(entries[0]["title"], "Discarded");
}

#[test]
fn rm_moves_body_to_archive() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "With Body");
    let body_path = dir.path().join(format!(".complex/issues/{}.md", id));
    std::fs::write(&body_path, "some content").unwrap();
    cx(&dir).args(["rm", &id]).assert().success();
    assert!(!body_path.exists());
    // body moved to archive dir
    let archived_body = dir.path().join(format!(".complex/archive/{}.md", id));
    assert!(archived_body.exists());
    assert_eq!(std::fs::read_to_string(archived_body).unwrap(), "some content");
}

#[test]
fn rm_json_output() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "To Remove");
    let out = cx(&dir).args(["--json", "rm", &id]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["removed"], true);
}

// ── cx find ───────────────────────────────────────────────────────────────

#[test]
fn find_matches_by_title() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "Implement auth");
    add(&dir, "Fix database bug");
    add(&dir, "Auth middleware");

    cx(&dir).args(["find", "auth"]).assert().success()
        .stdout(contains("Implement auth"))
        .stdout(contains("Auth middleware"));
}

#[test]
fn find_case_insensitive() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "JWT Token Handler");

    cx(&dir).args(["find", "jwt"]).assert().success()
        .stdout(contains("JWT Token Handler"));
}

#[test]
fn find_no_results() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    add(&dir, "Something");

    cx(&dir).args(["find", "nonexistent"]).assert().success()
        .stdout(contains("no nodes matching"));
}

#[test]
fn find_json_output() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Searchable Task");

    let out = cx(&dir).args(["--json", "find", "searchable"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["id"].as_str().unwrap(), id);
}

// ── claim state enforcement ──────────────────────────────────────────────

#[test]
fn claim_latent_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Latent Task");
    cx(&dir).args(["claim", &id, "--as", "agent"]).assert().failure()
        .stderr(contains("latent"))
        .stderr(contains("surface"));
}

// ── tree --json hierarchy ────────────────────────────────────────────────

#[test]
fn tree_json_is_hierarchical() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Root");
    let _child = new_child(&dir, &root, "Child");

    let out = cx(&dir).args(["--json", "tree"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["title"], "Root");
    assert_eq!(v[0]["children"][0]["title"], "Child");
}

#[test]
fn tree_json_scoped() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Root");
    let child = new_child(&dir, &root, "Child");
    let _grandchild = new_child(&dir, &child, "Grandchild");

    let out = cx(&dir).args(["--json", "tree", &child]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["title"], "Child");
    assert_eq!(v[0]["children"][0]["title"], "Grandchild");
}

// ── orphan detection in therapy ──────────────────────────────────────────

#[test]
fn therapy_detects_orphan_body_files() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    // Create an orphan body file (no matching node)
    std::fs::write(
        dir.path().join(".complex/issues/FAKE.md"),
        "orphan content",
    ).unwrap();

    cx(&dir).args(["therapy"]).assert().success()
        .stdout(contains("FAKE"))
        .stdout(contains("orphan"));
}

#[test]
fn therapy_orphan_json() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    std::fs::write(
        dir.path().join(".complex/issues/ZZZZ.md"),
        "orphan",
    ).unwrap();

    let out = cx(&dir).args(["--json", "therapy"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let orphan = v.as_array().unwrap().iter()
        .find(|e| e["reason"] == "orphan")
        .unwrap();
    assert_eq!(orphan["id"], "ZZZZ");
}

// ── edge validation ──────────────────────────────────────────────────────

#[test]
fn block_nonexistent_node_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Real Node");
    cx(&dir).args(["block", &id, "FAKE"]).assert().failure();
}

#[test]
fn relate_nonexistent_node_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Real Node");
    cx(&dir).args(["relate", &id, "NOPE"]).assert().failure();
}

// ── filed_by ──────────────────────────────────────────────────────────────────

#[test]
fn add_with_by_flag_sets_filed_by() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = cx(&dir)
        .args(["--json", "add", "Bug report", "--by", "ox:seguro"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    // Verify in show --json
    let show = cx(&dir).args(["--json", "show", id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(s["filed_by"], "ox:seguro");
}

#[test]
fn new_with_by_flag_sets_filed_by() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Parent");
    let out = cx(&dir)
        .args(["--json", "new", &root, "Child issue", "--by", "claude:myproj"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    let show = cx(&dir).args(["--json", "show", id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(s["filed_by"], "claude:myproj");
}

#[test]
fn cx_filed_by_env_var_fallback() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = cx(&dir)
        .env("CX_FILED_BY", "ox:seguro")
        .args(["--json", "add", "From env"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    let show = cx(&dir).args(["--json", "show", id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(s["filed_by"], "ox:seguro");
}

#[test]
fn by_flag_overrides_env_var() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = cx(&dir)
        .env("CX_FILED_BY", "env-agent")
        .args(["--json", "add", "Override test", "--by", "flag-agent"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    let show = cx(&dir).args(["--json", "show", id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(s["filed_by"], "flag-agent");
}

#[test]
fn filed_by_absent_when_not_set() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "No filer");

    let show = cx(&dir).args(["--json", "show", &id]).output().unwrap();
    let s: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert!(s["filed_by"].is_null());
}

#[test]
fn list_filtered_by_filed_by() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    cx(&dir).args(["add", "From ox", "--by", "ox:seguro"]).assert().success();
    cx(&dir).args(["add", "From claude", "--by", "claude:myproj"]).assert().success();
    cx(&dir).args(["add", "No filer"]).assert().success();

    let out = cx(&dir)
        .args(["--json", "list", "--filed-by", "ox:seguro"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["title"], "From ox");
    assert_eq!(arr[0]["filed_by"], "ox:seguro");
}

#[test]
fn filed_by_survives_roundtrip() {
    // Backward compat: filed_by is persisted in graph.json and survives load/save
    let dir = TempDir::new().unwrap();
    init(&dir);
    cx(&dir).args(["add", "Test", "--by", "ox:seguro"]).assert().success();

    let graph = graph_json(&dir);
    let node = &graph["nodes"][0];
    assert_eq!(node["filed_by"], "ox:seguro");
}

#[test]
fn old_graph_without_filed_by_loads_fine() {
    // Simulate an old graph.json that has no filed_by field (legacy migration)
    let dir = TempDir::new().unwrap();
    let root = dir.path().join(".complex");
    std::fs::create_dir_all(root.join("issues")).unwrap();
    std::fs::create_dir_all(root.join("archive")).unwrap();

    // Write a legacy graph.json WITHOUT nodes/ dir
    let graph = r#"{"version":1,"nodes":[{"id":"test","title":"Old node","state":"latent","shadowed":false,"part":null,"created_at":"2026-01-01T00:00:00+00:00","updated_at":"2026-01-01T00:00:00+00:00"}],"edges":[]}"#;
    std::fs::write(root.join("graph.json"), graph).unwrap();

    // Should load via migration and auto-create nodes/
    let out = cx(&dir).args(["--json", "show", "test"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["title"], "Old node");
    assert!(v["filed_by"].is_null());

    // Migration should have created per-node files
    assert!(root.join("nodes/test.json").exists());
    assert!(root.join("graph.json.bak").exists());
}

#[test]
fn filed_by_in_tree_json() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    cx(&dir).args(["add", "Root", "--by", "ox:seguro"]).assert().success();

    let out = cx(&dir).args(["--json", "tree"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v[0]["filed_by"], "ox:seguro");
}

#[test]
fn show_text_displays_filed_by() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = cx(&dir)
        .args(["--json", "add", "Bug", "--by", "ox:seguro"])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = v["id"].as_str().unwrap();

    cx(&dir).args(["show", id]).assert().success()
        .stdout(contains("filed by: ox:seguro"));
}

// ── cx move ──────────────────────────────────────────────────────────────────

#[test]
fn move_reparents_node_under_new_parent() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "Parent A");
    let b = add(&dir, "Parent B");
    let child = new_child(&dir, &a, "Child");

    cx(&dir).args(["move", &child, &b]).assert().success();

    // ID stays the same, parent field changes
    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    let child_node = nodes.iter().find(|n| n["id"].as_str().unwrap() == child).unwrap();
    assert_eq!(child_node["parent"].as_str().unwrap(), b);
}

#[test]
fn move_promotes_to_root() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let parent = add(&dir, "Parent");
    let child = new_child(&dir, &parent, "Child");

    cx(&dir).args(["move", &child, "--root"]).assert().success();

    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    let child_node = nodes.iter().find(|n| n["id"].as_str().unwrap() == child).unwrap();
    assert!(child_node.get("parent").is_none() || child_node["parent"].is_null(),
        "promoted to root should have no parent");
}

#[test]
fn move_carries_children_along() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let child = new_child(&dir, &a, "Child");
    let grandchild = new_child(&dir, &child, "Grandchild");

    cx(&dir).args(["move", &child, &b]).assert().success();

    // child's parent changed to b, grandchild's parent stays child (unchanged)
    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    let child_node = nodes.iter().find(|n| n["id"].as_str().unwrap() == child).unwrap();
    assert_eq!(child_node["parent"].as_str().unwrap(), b);
    let gc_node = nodes.iter().find(|n| n["id"].as_str().unwrap() == grandchild).unwrap();
    assert_eq!(gc_node["parent"].as_str().unwrap(), child, "grandchild parent unchanged");
}

#[test]
fn move_preserves_edges() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let blocker = add(&dir, "Blocker");
    let child = new_child(&dir, &a, "Child");

    cx(&dir).args(["block", &blocker, &child]).assert().success();
    cx(&dir).args(["move", &child, &b]).assert().success();

    // Edge still points to same ID (IDs don't change on move)
    let g = graph_json(&dir);
    let edges = g["edges"].as_array().unwrap();
    assert!(edges.iter().any(|e| e["to"].as_str().unwrap() == child), "edge should still reference same id");
}

#[test]
fn move_under_self_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let child = new_child(&dir, &a, "Child");

    cx(&dir).args(["move", &a, &child])
        .assert()
        .failure()
        .stderr(contains("descendant"));
}

#[test]
fn move_body_file_stays_in_place() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let child = new_child(&dir, &a, "Child");

    cx(&dir).args(["edit", &child, "--body", "hello world"]).assert().success();
    cx(&dir).args(["move", &child, &b]).assert().success();

    // ID doesn't change, so body file stays at same path
    let body_path = dir.path().join(format!(".complex/issues/{}.md", child));
    assert!(body_path.exists(), "body file should still be at original path");
    let content = std::fs::read_to_string(&body_path).unwrap();
    assert_eq!(content, "hello world");
}

#[test]
fn mv_alias_works() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let child = new_child(&dir, &a, "Child");

    cx(&dir).args(["mv", &child, &b]).assert().success();
}

// ── block propagation to children ────────────────────────────────────────────

#[test]
fn blocked_parent_hides_children_from_surface() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let blocker = add(&dir, "Blocker");
    let parent = add(&dir, "Parent");
    let child = new_child(&dir, &parent, "Child");
    surface_id(&dir, &parent);
    surface_id(&dir, &child);

    // Block the parent on the blocker
    cx(&dir).args(["block", &blocker, &parent]).assert().success();

    // Surface (ready list) should not show child
    let out = cx(&dir).args(["--json", "surface"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = v.as_array().unwrap().iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(!ids.contains(&child.as_str()), "child of blocked parent should not appear in surface");
    assert!(!ids.contains(&parent.as_str()), "blocked parent should not appear in surface");
}

#[test]
fn blocked_grandparent_hides_grandchildren_from_surface() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let blocker = add(&dir, "Blocker");
    let gp = add(&dir, "Grandparent");
    let parent = new_child(&dir, &gp, "Parent");
    let child = new_child(&dir, &parent, "Child");
    surface_id(&dir, &gp);
    surface_id(&dir, &parent);
    surface_id(&dir, &child);

    cx(&dir).args(["block", &blocker, &gp]).assert().success();

    let out = cx(&dir).args(["--json", "surface"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = v.as_array().unwrap().iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(!ids.contains(&child.as_str()), "grandchild of blocked grandparent should not appear in surface");
}

#[test]
fn claim_child_of_blocked_parent_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let blocker = add(&dir, "Blocker");
    let parent = add(&dir, "Parent");
    let child = new_child(&dir, &parent, "Child");
    surface_id(&dir, &parent);
    surface_id(&dir, &child);

    cx(&dir).args(["block", &blocker, &parent]).assert().success();

    cx(&dir).args(["claim", &child, "--as", "agent"])
        .assert()
        .failure()
        .stderr(contains("blocked"));
}

#[test]
fn unblocked_parent_children_reappear_in_surface() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let blocker = add(&dir, "Blocker");
    let parent = add(&dir, "Parent");
    let child = new_child(&dir, &parent, "Child");
    surface_id(&dir, &parent);
    surface_id(&dir, &child);

    cx(&dir).args(["block", &blocker, &parent]).assert().success();

    // Integrate the blocker to unblock
    surface_id(&dir, &blocker);
    claim(&dir, &blocker, "agent");
    integrate(&dir, &blocker);

    let out = cx(&dir).args(["--json", "surface"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = v.as_array().unwrap().iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&child.as_str()), "child should reappear after parent is unblocked");
}

// ── cx surface --all ───────────────────────────────────────────────────────────

#[test]
fn surface_all_promotes_eligible_latent_nodes() {
    // Three nodes: A (no blockers), B (no blockers), C (blocked by A).
    // surface --all should promote A and B but not C.
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let c = add(&dir, "C");
    cx(&dir).args(["block", &a, &c]).assert().success();

    let out = cx(&dir)
        .args(["--json", "surface", "--all"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = arr
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();

    // A and B should be surfaced, C should not
    assert!(ids.iter().any(|id| id.ends_with(&a)));
    assert!(ids.iter().any(|id| id.ends_with(&b)));
    assert!(!ids.iter().any(|id| id.ends_with(&c)));

    // Verify state in graph
    let g = graph_json(&dir);
    for node in g["nodes"].as_array().unwrap() {
        let id = node["id"].as_str().unwrap();
        if id.ends_with(&a) || id.ends_with(&b) {
            assert_eq!(node["state"], "ready", "{} should be ready", id);
        } else if id.ends_with(&c) {
            assert_eq!(node["state"], "latent", "{} should still be latent", id);
        }
    }
}

#[test]
fn surface_all_empty_when_all_blocked_or_ready() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    cx(&dir).args(["block", &a, &b]).assert().success();
    surface_id(&dir, &a); // A is now ready

    // Only B is latent, but it's blocked by A — nothing eligible
    let out = cx(&dir)
        .args(["--json", "surface", "--all"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(arr.as_array().unwrap().len(), 0);
}

#[test]
fn surface_all_human_readable_output() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "Alpha");
    let b = add(&dir, "Beta");

    cx(&dir).args(["surface", "--all"]).assert().success()
        .stdout(contains(&a))
        .stdout(contains(&b))
        .stdout(contains("→ ready"));
}

// ── tag tests ─────────────────────────────────────────────────────────────────

#[test]
fn tag_and_untag_basic() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "root");

    cx(&dir).args(["tag", &root, "phase:alpha"]).assert().success()
        .stdout(contains("+phase:alpha"));

    // Show should display the tag
    let out = cx(&dir).args(["--json", "show", &root]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"], serde_json::json!(["phase:alpha"]));
    assert_eq!(v["effective_tags"], serde_json::json!(["phase:alpha"]));

    // Untag
    cx(&dir).args(["untag", &root, "phase:alpha"]).assert().success()
        .stdout(contains("-phase:alpha"));

    let out = cx(&dir).args(["--json", "show", &root]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"], serde_json::json!([]));
}

#[test]
fn tag_idempotent() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "root");

    cx(&dir).args(["tag", &root, "v1"]).assert().success();
    cx(&dir).args(["tag", &root, "v1"]).assert().success();

    let out = cx(&dir).args(["--json", "show", &root]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"], serde_json::json!(["v1"]), "tag should not be duplicated");
}

#[test]
fn tag_propagation_parent_to_child() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "root");
    let child = new_child(&dir, &root, "child");
    let grandchild = new_child(&dir, &child, "grandchild");

    // Tag the root
    cx(&dir).args(["tag", &root, "phase:beta"]).assert().success();

    // Child and grandchild should inherit via effective_tags
    let out = cx(&dir).args(["--json", "show", &child]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"], serde_json::json!([]), "child has no own tags");
    assert_eq!(v["effective_tags"], serde_json::json!(["phase:beta"]), "child inherits parent tag");

    let out = cx(&dir).args(["--json", "show", &grandchild]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["effective_tags"], serde_json::json!(["phase:beta"]), "grandchild inherits through chain");
}

#[test]
fn tag_union_own_and_inherited() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "root");
    let child = new_child(&dir, &root, "child");

    cx(&dir).args(["tag", &root, "team:platform"]).assert().success();
    cx(&dir).args(["tag", &child, "sprint:3"]).assert().success();

    let out = cx(&dir).args(["--json", "show", &child]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"], serde_json::json!(["sprint:3"]), "child has own tag");
    let effective = v["effective_tags"].as_array().unwrap();
    assert!(effective.contains(&serde_json::json!("sprint:3")));
    assert!(effective.contains(&serde_json::json!("team:platform")));
}

#[test]
fn tag_on_add_and_new() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    let out = cx(&dir)
        .args(["--json", "add", "root", "--tag", "v1", "--tag", "alpha"])
        .output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let root = v["id"].as_str().unwrap().to_string();

    let out = cx(&dir).args(["--json", "show", &root]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let tags = v["tags"].as_array().unwrap();
    assert!(tags.contains(&serde_json::json!("v1")));
    assert!(tags.contains(&serde_json::json!("alpha")));

    // cx new with --tag
    let out = cx(&dir)
        .args(["--json", "new", &root, "child", "--tag", "sprint:1"])
        .output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let child = v["id"].as_str().unwrap().to_string();

    let out = cx(&dir).args(["--json", "show", &child]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"], serde_json::json!(["sprint:1"]));
}

#[test]
fn tag_list_filter() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "alpha-task");
    let b = add(&dir, "beta-task");

    cx(&dir).args(["tag", &a, "team:a"]).assert().success();
    cx(&dir).args(["tag", &b, "team:b"]).assert().success();

    let out = cx(&dir).args(["--json", "list", "--tag", "team:a"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = v.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&a.as_str()));
    assert!(!ids.contains(&b.as_str()));
}

#[test]
fn tag_list_filter_inherited() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "root");
    let child = new_child(&dir, &root, "child");

    cx(&dir).args(["tag", &root, "project:x"]).assert().success();

    // list --tag project:x should find the child too (inherited)
    let out = cx(&dir).args(["--json", "list", "--tag", "project:x"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = v.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&root.as_str()));
    assert!(ids.contains(&child.as_str()));
}

#[test]
fn tags_command_lists_all() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "a");
    let b = add(&dir, "b");

    cx(&dir).args(["tag", &a, "team:a"]).assert().success();
    cx(&dir).args(["tag", &b, "team:b"]).assert().success();

    let out = cx(&dir).args(["--json", "tags"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let tags: Vec<&str> = v.as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
    assert!(tags.contains(&"team:a"));
    assert!(tags.contains(&"team:b"));
}

#[test]
fn tags_command_shows_inherited() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "root");
    let child = new_child(&dir, &root, "child");

    cx(&dir).args(["tag", &root, "phase:1"]).assert().success();
    cx(&dir).args(["tag", &child, "own:tag"]).assert().success();

    let out = cx(&dir).args(["--json", "tags", &child]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let own: Vec<&str> = v["own"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
    let effective: Vec<&str> = v["effective"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
    assert_eq!(own, vec!["own:tag"]);
    assert!(effective.contains(&"own:tag"));
    assert!(effective.contains(&"phase:1"));
}

#[test]
fn tag_denormalized_on_archive() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "root");
    let child = new_child(&dir, &root, "child");

    cx(&dir).args(["tag", &root, "release:v1"]).assert().success();

    // Integrate then archive the child — tags baked in at archive time
    surface_id(&dir, &child);
    integrate(&dir, &child);
    archive(&dir, &child);

    // Read archived node file and verify the child has the inherited tag
    let archive_path = dir.path().join(format!(".complex/archive/nodes/{}.json", child));
    let raw = std::fs::read_to_string(&archive_path).unwrap();
    let archived: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(archived["id"].as_str().unwrap(), child);
    let tags: Vec<&str> = archived["tags"].as_array().unwrap().iter()
        .map(|t| t.as_str().unwrap()).collect();
    assert!(tags.contains(&"release:v1"), "archived node should have inherited tag baked in");
}

#[test]
fn tag_tree_filter() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "tagged-root");
    let b = add(&dir, "untagged-root");
    let _child = new_child(&dir, &a, "child-of-tagged");

    cx(&dir).args(["tag", &a, "focus"]).assert().success();

    let out = cx(&dir).args(["--json", "tree", "--tag", "focus"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = v.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&a.as_str()), "tagged root should appear");
    assert!(!ids.iter().any(|id| *id == b), "untagged root should be filtered out");
}

#[test]
fn tag_find_filter() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "deploy service");
    let b = add(&dir, "deploy database");

    cx(&dir).args(["tag", &a, "team:sre"]).assert().success();
    cx(&dir).args(["tag", &b, "team:dba"]).assert().success();

    let out = cx(&dir).args(["--json", "find", "deploy", "--tag", "team:sre"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = v.as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&a.as_str()));
    assert!(!ids.contains(&b.as_str()));
}

#[test]
fn tag_sorted_on_node() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "root");

    cx(&dir).args(["tag", &root, "z-tag"]).assert().success();
    cx(&dir).args(["tag", &root, "a-tag"]).assert().success();

    let out = cx(&dir).args(["--json", "show", &root]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"], serde_json::json!(["a-tag", "z-tag"]), "tags should be sorted");
}

#[test]
fn tag_serde_compat_no_tags_field() {
    // Legacy graph.json without tags field should migrate and load (serde default)
    let dir = TempDir::new().unwrap();
    let root = dir.path().join(".complex");
    std::fs::create_dir_all(root.join("issues")).unwrap();
    std::fs::create_dir_all(root.join("archive")).unwrap();

    // Write a legacy graph.json WITHOUT nodes/ dir
    let graph = r#"{
        "version": 1,
        "nodes": [{
            "id": "ABCD",
            "title": "legacy node",
            "state": "latent",
            "shadowed": false,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }],
        "edges": []
    }"#;
    std::fs::write(root.join("graph.json"), graph).unwrap();

    // Should migrate and load fine with empty tags
    let out = cx(&dir).args(["--json", "show", "ABCD"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"], serde_json::json!([]));
}

// ── ID collision detection ───────────────────────────────────────────────────

#[test]
fn add_many_nodes_no_duplicate_ids() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    let mut ids = std::collections::HashSet::new();
    for i in 0..100 {
        let id = add(&dir, &format!("node-{}", i));
        assert!(ids.insert(id.clone()), "duplicate id: {}", id);
    }
}

#[test]
fn new_child_no_duplicate_after_archive() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    let root = add(&dir, "root");
    let child = new_child(&dir, &root, "child-1");

    // Surface, claim, integrate, archive the child
    surface_id(&dir, &child);
    claim(&dir, &child, "test");
    integrate(&dir, &child);
    cx(&dir).args(["archive", "--ids", &child]).assert().success();

    // Create many more children — none should collide with the archived child's
    // leaf segment (probabilistically; 50 nodes is safe against 14.8M space)
    let mut ids = std::collections::HashSet::new();
    ids.insert(child.clone());
    for i in 0..50 {
        let c = new_child(&dir, &root, &format!("child-{}", i + 2));
        assert!(ids.insert(c.clone()), "duplicate full id");
    }
}

// ── comments ─────────────────────────────────────────────────────────────────

#[test]
fn comment_append_and_list() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "comment target");

    // Append a comment with inline body
    cx(&dir)
        .args(["comment", &id, "--as", "alice", "--tag", "proposal", "my plan"])
        .assert()
        .success()
        .stdout(contains("comment").and(contains(&id)));

    // Append a second comment (no tag)
    cx(&dir)
        .args(["comment", &id, "--as", "bob", "just a note"])
        .assert()
        .success();

    // List all comments
    let out = cx(&dir)
        .args(["--json", "comments", &id])
        .output()
        .unwrap();
    assert!(out.status.success());
    let comments: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0]["author"], "alice");
    assert_eq!(comments[0]["tag"], "proposal");
    assert_eq!(comments[0]["body"], "my plan");
    assert_eq!(comments[1]["author"], "bob");
    assert!(comments[1]["tag"].is_null());

    // Filter by tag
    let out = cx(&dir)
        .args(["--json", "comments", &id, "--tag", "proposal"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let filtered: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0]["author"], "alice");
}

#[test]
fn comment_from_file() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "file comment test");

    let body_file = dir.path().join("comment-body.md");
    std::fs::write(&body_file, "# Proposal\n\nDo the thing.\n").unwrap();

    cx(&dir)
        .args([
            "comment", &id,
            "--as", "agent",
            "--tag", "proposal",
            "--file", body_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let out = cx(&dir)
        .args(["--json", "comments", &id])
        .output()
        .unwrap();
    let comments: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "# Proposal\n\nDo the thing.\n");
}

#[test]
fn comment_edit() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "edit comment test");

    // Append a comment
    let out = cx(&dir)
        .args(["--json", "comment", &id, "--as", "alice", "--tag", "proposal", "original"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ts = v["timestamp"].as_str().unwrap().to_string();

    // Edit it
    cx(&dir)
        .args(["comment", &id, "--edit", &ts, "updated body"])
        .assert()
        .success()
        .stdout(contains("edited"));

    // Verify
    let out = cx(&dir)
        .args(["--json", "comments", &id])
        .output()
        .unwrap();
    let comments: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "updated body");
    // tag preserved
    assert_eq!(comments[0]["tag"], "proposal");
}

#[test]
fn comment_remove() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "rm comment test");

    // Append two comments
    let out1 = cx(&dir)
        .args(["--json", "comment", &id, "--as", "a", "first"])
        .output()
        .unwrap();
    let ts1: serde_json::Value = serde_json::from_slice(&out1.stdout).unwrap();
    let ts1 = ts1["timestamp"].as_str().unwrap().to_string();

    cx(&dir)
        .args(["comment", &id, "--as", "b", "second"])
        .assert()
        .success();

    // Remove the first
    cx(&dir)
        .args(["comment", &id, "--rm", &ts1])
        .assert()
        .success()
        .stdout(contains("removed"));

    // Verify only one remains
    let out = cx(&dir)
        .args(["--json", "comments", &id])
        .output()
        .unwrap();
    let comments: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "second");
}

#[test]
fn comment_empty_body_rejected() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "empty comment test");

    // Empty inline body should fail
    cx(&dir)
        .args(["comment", &id])
        .assert()
        .failure()
        .stderr(contains("empty"));
}

#[test]
fn comment_not_in_show() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "show without comments");

    cx(&dir)
        .args(["comment", &id, "--as", "alice", "--tag", "review", "LGTM"])
        .assert()
        .success();

    // cx show should NOT include comments
    cx(&dir)
        .args(["show", &id])
        .assert()
        .success()
        .stdout(contains("show without comments"))
        .stdout(contains("LGTM").not());

    // cx comments is the way to read them
    let out = cx(&dir)
        .args(["--json", "comments", &id])
        .output()
        .unwrap();
    let comments: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "LGTM");
}

#[test]
fn comment_persists_across_load() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "persistence test");

    cx(&dir)
        .args(["comment", &id, "--as", "bot", "hello world"])
        .assert()
        .success();

    // Comments file should exist
    let comments_path = dir
        .path()
        .join(".complex/issues")
        .join(format!("{}.comments.json", id));
    assert!(comments_path.exists());

    // Load via a fresh cx invocation
    let out = cx(&dir)
        .args(["--json", "comments", &id])
        .output()
        .unwrap();
    let comments: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "hello world");
}

#[test]
fn comment_rm_nonexistent_timestamp_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "bad rm test");

    cx(&dir)
        .args(["comment", &id, "--rm", "2099-01-01T00:00:00Z"])
        .assert()
        .failure()
        .stderr(contains("no comment with timestamp"));
}

// ── archive edge preservation + unarchive ────────────────────────────────────

/// Scenario 1: Archive one node, unarchive it — edges round-trip cleanly.
#[test]
fn unarchive_single_node_restores_edges() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let c = add(&dir, "C");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    surface_id(&dir, &c);
    block(&dir, &a, &b);
    block(&dir, &a, &c);

    // Archive A
    claim(&dir, &a, "test");
    integrate(&dir, &a);
    archive(&dir, &a);

    // A's edges should be in archived edges, not live graph
    assert!(!graph_node_ids(&dir).contains(&a));
    let live = graph_edges(&dir);
    assert!(!has_edge(&live, &a, &b));
    assert!(!has_edge(&live, &a, &c));
    let stashed = archived_edges(&dir);
    assert!(has_edge(&stashed, &a, &b));
    assert!(has_edge(&stashed, &a, &c));

    // Unarchive A
    unarchive(&dir, &a);

    // A is back, edges restored
    assert!(graph_node_ids(&dir).contains(&a));
    let live = graph_edges(&dir);
    assert!(has_edge(&live, &a, &b));
    assert!(has_edge(&live, &a, &c));
    assert!(archived_edges(&dir).is_empty());
}

/// Scenario 2: Archive both endpoints, unarchive in reverse order — full recovery.
#[test]
fn unarchive_both_endpoints_reverse_order() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let c = add(&dir, "C");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    surface_id(&dir, &c);
    block(&dir, &a, &b);
    block(&dir, &a, &c);
    block(&dir, &b, &c);

    // Archive A, then B
    claim(&dir, &a, "t");
    integrate(&dir, &a);
    archive(&dir, &a);
    claim(&dir, &b, "t");
    integrate(&dir, &b);
    archive(&dir, &b);

    // Only C remains, no live edges
    assert_eq!(graph_node_ids(&dir), vec![c.clone()]);
    assert!(graph_edges(&dir).is_empty());
    // All three edges in archive
    let stashed = archived_edges(&dir);
    assert_eq!(stashed.len(), 3);

    // Unarchive B first
    unarchive(&dir, &b);
    let live = graph_edges(&dir);
    // B→C should be restored (both live), A→B should not (A still archived)
    assert!(has_edge(&live, &b, &c));
    assert!(!has_edge(&live, &a, &b));
    // A→B and A→C still in archive
    let stashed = archived_edges(&dir);
    assert_eq!(stashed.len(), 2);
    assert!(has_edge(&stashed, &a, &b));
    assert!(has_edge(&stashed, &a, &c));

    // Unarchive A
    unarchive(&dir, &a);
    let live = graph_edges(&dir);
    assert!(has_edge(&live, &a, &b));
    assert!(has_edge(&live, &a, &c));
    assert!(has_edge(&live, &b, &c));
    assert!(archived_edges(&dir).is_empty());
}

/// Scenario 3: Archive all three, unarchive in arbitrary order — full recovery.
#[test]
fn unarchive_all_three_arbitrary_order() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let c = add(&dir, "C");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    surface_id(&dir, &c);
    block(&dir, &a, &b);
    block(&dir, &a, &c);
    block(&dir, &b, &c);

    // Archive all
    for id in [&a, &b, &c] {
        claim(&dir, id, "t");
        integrate(&dir, id);
        archive(&dir, id);
    }
    assert!(graph_node_ids(&dir).is_empty());
    assert!(graph_edges(&dir).is_empty());

    // Unarchive C, then A, then B
    unarchive(&dir, &c);
    assert!(graph_edges(&dir).is_empty()); // no partner live yet

    unarchive(&dir, &a);
    let live = graph_edges(&dir);
    assert!(has_edge(&live, &a, &c)); // A→C restored
    assert!(!has_edge(&live, &a, &b)); // B still archived

    unarchive(&dir, &b);
    let live = graph_edges(&dir);
    assert!(has_edge(&live, &a, &b));
    assert!(has_edge(&live, &a, &c));
    assert!(has_edge(&live, &b, &c));
    assert!(archived_edges(&dir).is_empty());
}

/// Scenario 4: Partial unarchive — some nodes stay archived, edges stay in pool.
#[test]
fn partial_unarchive_edges_stay_in_pool() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let c = add(&dir, "C");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    surface_id(&dir, &c);
    block(&dir, &a, &b);
    block(&dir, &a, &c);
    block(&dir, &b, &c);

    // Archive A and B
    claim(&dir, &a, "t");
    integrate(&dir, &a);
    archive(&dir, &a);
    claim(&dir, &b, "t");
    integrate(&dir, &b);
    archive(&dir, &b);

    // Unarchive only A
    unarchive(&dir, &a);
    let live = graph_edges(&dir);
    assert!(has_edge(&live, &a, &c)); // A→C restored (both live)
    assert!(!has_edge(&live, &a, &b)); // B still archived

    // A→B and B→C stay in archived edges
    let stashed = archived_edges(&dir);
    assert!(has_edge(&stashed, &a, &b));
    assert!(has_edge(&stashed, &b, &c));
}

/// Scenario 5: New edge added after archive — no conflict on unarchive.
#[test]
fn new_edge_after_archive_no_conflict() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    let c = add(&dir, "C");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    surface_id(&dir, &c);
    block(&dir, &a, &b);

    // Archive A
    claim(&dir, &a, "t");
    integrate(&dir, &a);
    archive(&dir, &a);

    // Add a new edge B→C while A is archived
    block(&dir, &b, &c);

    // Unarchive A
    unarchive(&dir, &a);
    let live = graph_edges(&dir);
    assert!(has_edge(&live, &a, &b)); // restored
    assert!(has_edge(&live, &b, &c)); // was added while A archived
}

/// Scenario 6: cx rm scrubs archived edges referencing the removed node.
#[test]
fn rm_scrubs_archived_edges() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    let b = add(&dir, "B");
    surface_id(&dir, &a);
    surface_id(&dir, &b);
    block(&dir, &a, &b);

    // Archive A (edge A→B becomes dormant in A's archived node file)
    claim(&dir, &a, "t");
    integrate(&dir, &a);
    archive(&dir, &a);
    assert!(has_edge(&archived_edges(&dir), &a, &b));

    // rm B — should also scrub A→B from archived edges
    cx(&dir).args(["rm", &b]).assert().success();

    // Archived edges referencing B should be gone
    let stashed = archived_edges(&dir);
    assert!(!has_edge(&stashed, &a, &b));
}

/// Unarchived node returns in integrated state.
#[test]
fn unarchive_restores_as_integrated() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    surface_id(&dir, &a);
    claim(&dir, &a, "t");
    integrate(&dir, &a);
    archive(&dir, &a);

    let out = cx(&dir)
        .args(["--json", "unarchive", &a])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["id"].as_str().unwrap(), a);

    // Verify state is integrated
    let g = graph_json(&dir);
    let node = g["nodes"].as_array().unwrap().iter().find(|n| n["id"] == a).unwrap();
    assert_eq!(node["state"], "integrated");
}

/// Unarchive restores body and comments files.
#[test]
fn unarchive_restores_body_and_comments() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let a = add(&dir, "A");
    surface_id(&dir, &a);

    // Set a body
    cx(&dir).args(["edit", &a, "--body", "my body content"]).assert().success();
    // Add a comment
    cx(&dir).args(["comment", &a, "hello"]).assert().success();

    claim(&dir, &a, "t");
    integrate(&dir, &a);
    archive(&dir, &a);

    // Files should be in archive/
    assert!(dir.path().join(format!(".complex/archive/{}.md", a)).exists());
    assert!(dir.path().join(format!(".complex/archive/{}.comments.json", a)).exists());
    assert!(!dir.path().join(format!(".complex/issues/{}.md", a)).exists());
    assert!(!dir.path().join(format!(".complex/issues/{}.comments.json", a)).exists());

    unarchive(&dir, &a);

    // Files should be back in issues/
    assert!(dir.path().join(format!(".complex/issues/{}.md", a)).exists());
    assert!(!dir.path().join(format!(".complex/archive/{}.md", a)).exists());
    let body = std::fs::read_to_string(dir.path().join(format!(".complex/issues/{}.md", a))).unwrap();
    assert!(body.contains("my body content"));
}

/// Unarchive a non-existent ID fails gracefully.
#[test]
fn unarchive_missing_id_fails() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    cx(&dir).args(["unarchive", "ZZZZ"]).assert().failure();
}

// ── cx log (git-backed) ─────────────────────────────────────────────────────

/// Helper: set up a git repo in the temp dir so cx log can read history.
fn git_init(dir: &TempDir) {
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();
}

fn git_commit(dir: &TempDir, msg: &str) {
    std::process::Command::new("git")
        .args(["add", ".complex/"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", msg, "--allow-empty"])
        .current_dir(dir.path())
        .output()
        .unwrap();
}

fn git_rev(dir: &TempDir) -> String {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn log_shows_node_changes() {
    let dir = TempDir::new().unwrap();
    git_init(&dir);
    init(&dir);
    let id = add(&dir, "Test task");
    git_commit(&dir, "create task");

    let out = cx(&dir)
        .args(["--json", "log", "--limit", "5"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let commits: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert!(!commits.is_empty());

    // The commit should have changes listing the created node
    let changes = commits[0]["changes"].as_array().unwrap();
    let created: Vec<_> = changes.iter()
        .filter(|c| c["action"] == "created")
        .collect();
    assert!(!created.is_empty(), "expected a 'created' change for the new node");
    assert!(created.iter().any(|c| c["node_id"].as_str() == Some(&id)));
}

#[test]
fn log_shows_state_transition() {
    let dir = TempDir::new().unwrap();
    git_init(&dir);
    init(&dir);
    let id = add(&dir, "Transition test");
    git_commit(&dir, "create task");

    surface_id(&dir, &id);
    git_commit(&dir, "surface task");

    let out = cx(&dir)
        .args(["--json", "log", "--limit", "1"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let commits: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    let changes = commits[0]["changes"].as_array().unwrap();
    let state_change = changes.iter()
        .find(|c| c["node_id"].as_str() == Some(&id))
        .expect("expected a change for our node");
    assert_eq!(state_change["action"], "modified");
    // Should show state transition
    let fields = state_change["fields"].as_object().unwrap();
    let state = fields.get("state").unwrap();
    assert_eq!(state["from"], "latent");
    assert_eq!(state["to"], "ready");
}

#[test]
fn log_since_filters_commits() {
    let dir = TempDir::new().unwrap();
    git_init(&dir);
    init(&dir);
    let _a = add(&dir, "First");
    git_commit(&dir, "first");

    let cursor = git_rev(&dir);

    let _b = add(&dir, "Second");
    git_commit(&dir, "second");

    // --since should only show commits after the cursor
    let out = cx(&dir)
        .args(["--json", "log", "--since", &cursor])
        .output()
        .unwrap();
    assert!(out.status.success());
    let commits: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(commits.len(), 1, "expected exactly 1 commit after cursor");
    assert_eq!(commits[0]["subject"].as_str().unwrap(), "second");
}

#[test]
fn log_shows_comment_and_body_changes() {
    let dir = TempDir::new().unwrap();
    git_init(&dir);
    init(&dir);
    let id = add(&dir, "Documented task");
    git_commit(&dir, "create task");

    // Add body and comment
    cx(&dir).args(["edit", &id, "--body", "Some details"]).assert().success();
    cx(&dir).args(["comment", &id, "--as", "alice", "a note"]).assert().success();
    git_commit(&dir, "add body and comment");

    let out = cx(&dir)
        .args(["--json", "log", "--limit", "1"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let commits: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    let changes = commits[0]["changes"].as_array().unwrap();
    let actions: Vec<&str> = changes.iter().filter_map(|c| c["action"].as_str()).collect();
    assert!(actions.contains(&"body_added"), "expected body_added, got {:?}", actions);
    assert!(actions.contains(&"comments_changed"), "expected comments_changed, got {:?}", actions);
}

#[test]
fn log_no_git_repo_fails_gracefully() {
    let dir = TempDir::new().unwrap();
    // No git init — just cx init
    init(&dir);

    cx(&dir).args(["log"]).assert().failure();
}

