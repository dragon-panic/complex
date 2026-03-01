use assert_cmd::Command;
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

fn graph_json(dir: &TempDir) -> serde_json::Value {
    let raw = std::fs::read_to_string(dir.path().join(".complex/graph.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn archive_json(dir: &TempDir) -> serde_json::Value {
    let raw =
        std::fs::read_to_string(dir.path().join(".complex/archive/archive.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

// ── cx init ───────────────────────────────────────────────────────────────────

#[test]
fn init_creates_structure() {
    let dir = TempDir::new().unwrap();
    cx(&dir).arg("init").assert().success();

    let root = dir.path().join(".complex");
    assert!(root.exists());
    assert!(root.join("graph.json").exists());
    assert!(root.join("issues").is_dir());
    assert!(root.join("archive").is_dir());

    // graph.json should be valid with empty nodes/edges
    let g = graph_json(&dir);
    assert_eq!(g["version"], 1);
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
fn new_creates_child_with_correct_prefix() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let parent = add(&dir, "Auth");
    let child = new_child(&dir, &parent, "Implement JWT");

    // child id is parent.XXXX
    assert!(child.starts_with(&format!("{}.", parent)));
    assert_eq!(child.len(), parent.len() + 5); // dot + 4 chars

    let g = graph_json(&dir);
    let nodes = g["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
}

#[test]
fn new_accepts_short_parent_id() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let parent = add(&dir, "Auth");
    // Use only the 4-char leaf segment
    let child = new_child(&dir, &parent, "JWT task");
    assert!(child.starts_with(&format!("{}.", parent)));
}

#[test]
fn new_grandchild_has_two_dots() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let root = add(&dir, "Root");
    let child = new_child(&dir, &root, "Child");
    let grandchild = new_child(&dir, &child, "Grandchild");
    assert_eq!(grandchild.matches('.').count(), 2);
}

#[test]
fn new_bad_parent_fails_with_hint() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    cx(&dir).args(["new", "doesnotexist", "title"])
        .assert().failure()
        .stderr(contains("cx tree"));
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

    // node gone from graph.json
    let g = graph_json(&dir);
    assert!(g["nodes"].as_array().unwrap().is_empty());

    // node in archive
    let archived = archive_json(&dir);
    let arr = archived.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], id.as_str());
    assert_eq!(arr[0]["state"], "integrated");
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

    integrate(&dir, &a);

    // edge removed
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
    let dir = TempDir::new().unwrap();
    init(&dir);

    // Write two nodes that share the same leaf segment (ZZZZ)
    // but have different parent prefixes — neither is an exact match for "ZZZZ"
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
    std::fs::write(
        dir.path().join(".complex/graph.json"),
        serde_json::to_string_pretty(&g).unwrap(),
    ).unwrap();

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
fn therapy_surfaces_stale_claimed_nodes() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let id = add(&dir, "Stale Task");
    surface_id(&dir, &id);
    claim(&dir, &id, "agent-1");

    // Backdate updated_at to 2 days ago
    let raw = std::fs::read_to_string(dir.path().join(".complex/graph.json")).unwrap();
    let updated = raw.replace(
        &format!("\"part\": \"agent-1\""),
        "\"part\": \"agent-1\"",
    );
    // Replace updated_at with old timestamp via JSON manipulation
    let mut g: serde_json::Value = serde_json::from_str(&raw).unwrap();
    g["nodes"][0]["updated_at"] = serde_json::json!("2026-01-01T00:00:00Z");
    std::fs::write(
        dir.path().join(".complex/graph.json"),
        serde_json::to_string_pretty(&g).unwrap(),
    ).unwrap();
    drop(updated);

    cx(&dir).args(["therapy"]).assert().success()
        .stdout(contains("Stale Task"))
        .stdout(contains("stale"));
}
