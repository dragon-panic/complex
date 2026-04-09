use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use fs2::FileExt;

use crate::model::{Edge, Graph, Node, OutgoingEdge};

const COMPLEX_DIR: &str = ".complex";
const GRAPH_FILE: &str = "graph.json";
const NODES_DIR: &str = "nodes";
const ISSUES_DIR: &str = "issues";
const ARCHIVE_DIR: &str = "archive";


// ── project location ──────────────────────────────────────────────────────────

pub fn find_root() -> Result<PathBuf> {
    if let Ok(cx_dir) = std::env::var("CX_DIR") {
        let p = PathBuf::from(cx_dir);
        if p.join(NODES_DIR).exists() || p.join(GRAPH_FILE).exists() {
            return Ok(p);
        }
        bail!("CX_DIR is set to {} but no nodes/ or graph.json found there — run cx init", p.display());
    }
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(COMPLEX_DIR).exists() {
            return Ok(dir.join(COMPLEX_DIR));
        }
        if !dir.pop() {
            bail!("not in a complex project (no .complex/ found — run cx init)");
        }
    }
}

pub fn init(cwd: &Path) -> Result<PathBuf> {
    let root = if let Ok(cx_dir) = std::env::var("CX_DIR") {
        PathBuf::from(cx_dir)
    } else {
        cwd.join(COMPLEX_DIR)
    };
    if root.exists() {
        bail!("{} already exists", root.display());
    }
    fs::create_dir_all(root.join(NODES_DIR))?;
    fs::create_dir_all(root.join(ISSUES_DIR))?;
    fs::create_dir_all(root.join(ARCHIVE_DIR))?;
    Ok(root)
}

// ── graph load / save ─────────────────────────────────────────────────────────

pub fn load(root: &Path) -> Result<Graph> {
    let nodes_dir = root.join(NODES_DIR);
    let graph_path = root.join(GRAPH_FILE);

    if nodes_dir.exists() {
        load_per_node(root)
    } else if graph_path.exists() {
        // Legacy: migrate from graph.json
        let mut graph = load_legacy(root)?;
        fs::create_dir_all(&nodes_dir)?;
        save(root, &graph)?;
        // Back up the old graph.json
        fs::rename(&graph_path, root.join("graph.json.bak"))?;
        // Reload body/comments (save doesn't persist those from the in-memory graph)
        load_bodies_and_comments(root, &mut graph)?;
        Ok(graph)
    } else {
        bail!("no nodes/ or graph.json found in {}", root.display());
    }
}

fn load_legacy(root: &Path) -> Result<Graph> {
    let path = root.join(GRAPH_FILE);
    let json = fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let mut graph: Graph = serde_json::from_str(&json)?;

    // Populate parent field from dot-separated ID if not already set
    for node in &mut graph.nodes {
        if node.parent.is_none()
            && let Some(dot) = node.id.rfind('.')
        {
            node.parent = Some(node.id[..dot].to_string());
        }
    }

    load_bodies_and_comments(root, &mut graph)?;
    Ok(graph)
}

fn load_per_node(root: &Path) -> Result<Graph> {
    let nodes_dir = root.join(NODES_DIR);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for entry in fs::read_dir(&nodes_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let json = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let node: Node = serde_json::from_str(&json)
            .with_context(|| format!("parsing {}", path.display()))?;
        // Expand outgoing edges into graph-level edges
        for oe in &node.outgoing_edges {
            edges.push(Edge {
                from: node.id.clone(),
                to: oe.to.clone(),
                edge_type: oe.edge_type.clone(),
            });
        }
        nodes.push(node);
    }

    // Filter out dormant edges (target not in the loaded graph)
    let live_ids: std::collections::HashSet<&str> =
        nodes.iter().map(|n| n.id.as_str()).collect();
    edges.retain(|e| live_ids.contains(e.to.as_str()));

    let mut graph = Graph {
        version: 1,
        nodes,
        edges,
    };

    load_bodies_and_comments(root, &mut graph)?;
    Ok(graph)
}

fn load_bodies_and_comments(root: &Path, graph: &mut Graph) -> Result<()> {
    let issues = root.join(ISSUES_DIR);
    for node in &mut graph.nodes {
        let md = issues.join(format!("{}.md", node.id));
        if md.exists() {
            node.body = Some(fs::read_to_string(&md)?);
        }
        let comments_path = issues.join(format!("{}.comments.json", node.id));
        if comments_path.exists() {
            node.comments = serde_json::from_str(&fs::read_to_string(&comments_path)?)?;
        }
    }
    Ok(())
}

pub fn save(root: &Path, graph: &Graph) -> Result<()> {
    let lock_path = root.join("cx.lock");
    let lock_file = File::create(&lock_path)?;
    lock_file.lock_exclusive().context("acquiring cx.lock")?;

    let nodes_dir = root.join(NODES_DIR);
    fs::create_dir_all(&nodes_dir)?;

    // Build set of live node IDs
    let live_ids: std::collections::HashSet<&str> =
        graph.nodes.iter().map(|n| n.id.as_str()).collect();

    // Write each node file with its outgoing edges
    for node in &graph.nodes {
        let outgoing: Vec<OutgoingEdge> = graph
            .edges
            .iter()
            .filter(|e| e.from == node.id)
            .map(|e| OutgoingEdge {
                to: e.to.clone(),
                edge_type: e.edge_type.clone(),
            })
            .collect();
        // Preserve dormant edges (target not live) from the node's loaded outgoing_edges
        let mut all_outgoing = outgoing;
        for oe in &node.outgoing_edges {
            if !live_ids.contains(oe.to.as_str())
                && !all_outgoing.iter().any(|e| e.to == oe.to && e.edge_type == oe.edge_type)
            {
                all_outgoing.push(oe.clone());
            }
        }
        let mut save_node = node.clone();
        save_node.outgoing_edges = all_outgoing;
        let json = serde_json::to_string_pretty(&save_node)?;
        fs::write(nodes_dir.join(format!("{}.json", node.id)), json)?;
    }

    // Remove node files for nodes no longer in the graph
    for entry in fs::read_dir(&nodes_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && path.extension().and_then(|e| e.to_str()) == Some("json")
            && !live_ids.contains(stem)
        {
            fs::remove_file(&path)?;
        }
    }

    // Write body and comment files
    let issues = root.join(ISSUES_DIR);
    fs::create_dir_all(&issues)?;
    for node in &graph.nodes {
        if let Some(body) = &node.body {
            fs::write(issues.join(format!("{}.md", node.id)), body)?;
        }
        let comments_path = issues.join(format!("{}.comments.json", node.id));
        if node.comments.is_empty() {
            if comments_path.exists() {
                fs::remove_file(&comments_path)?;
            }
        } else {
            fs::write(&comments_path, serde_json::to_string_pretty(&node.comments)?)?;
        }
    }

    lock_file.unlock()?;
    Ok(())
}

// ── issue bodies ──────────────────────────────────────────────────────────────

pub fn write_body(root: &Path, id: &str, body: &str) -> Result<()> {
    let dir = root.join(ISSUES_DIR);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(format!("{}.md", id)), body)?;
    Ok(())
}

pub fn read_body(root: &Path, id: &str) -> Result<Option<String>> {
    let path = root.join(ISSUES_DIR).join(format!("{}.md", id));
    if path.exists() {
        Ok(Some(fs::read_to_string(path)?))
    } else {
        Ok(None)
    }
}

// ── archive ───────────────────────────────────────────────────────────────────

const ARCHIVE_NODES_DIR: &str = "nodes";

/// Archive a node: move its file to archive/nodes/{id}.json.
/// Outgoing edges travel with the node. Incoming edges from other nodes
/// stay dormant in those nodes' files (filtered out on load).
pub fn archive_node(root: &Path, graph: &mut Graph, id: &str) -> Result<()> {
    let pos = graph
        .nodes
        .iter()
        .position(|n| n.id == id)
        .with_context(|| format!("node '{}' not found", id))?;
    let node = graph.nodes.remove(pos);

    // Collect this node's outgoing edges before removing them from graph
    let (outgoing, keep): (Vec<_>, Vec<_>) =
        graph.edges.drain(..).partition(|e| e.from == id);
    // Also remove incoming edges (from other nodes to this one) from graph.edges
    let (_, keep): (Vec<_>, Vec<_>) =
        keep.into_iter().partition(|e| e.to == id);
    graph.edges = keep;

    // Write archived node file with its outgoing edges
    let archive_nodes = root.join(ARCHIVE_DIR).join(ARCHIVE_NODES_DIR);
    fs::create_dir_all(&archive_nodes)?;
    let mut save_node = node;
    save_node.outgoing_edges = outgoing
        .into_iter()
        .map(|e| OutgoingEdge {
            to: e.to,
            edge_type: e.edge_type,
        })
        .collect();
    let json = serde_json::to_string_pretty(&save_node)?;
    fs::write(archive_nodes.join(format!("{}.json", id)), json)?;

    // Remove live node file (save() would also clean it up, but be explicit)
    let live_path = root.join(NODES_DIR).join(format!("{}.json", id));
    if live_path.exists() {
        fs::remove_file(&live_path)?;
    }

    // Move markdown body to archive dir
    let src = root.join(ISSUES_DIR).join(format!("{}.md", id));
    let dst = root.join(ARCHIVE_DIR).join(format!("{}.md", id));
    if src.exists() {
        fs::rename(src, dst)?;
    }

    // Move comments file to archive dir
    let csrc = root.join(ISSUES_DIR).join(format!("{}.comments.json", id));
    let cdst = root.join(ARCHIVE_DIR).join(format!("{}.comments.json", id));
    if csrc.exists() {
        fs::rename(csrc, cdst)?;
    }

    Ok(())
}

/// Restore a node from the archive back into the graph (as integrated).
/// Dormant edges from other live nodes auto-reconnect on next load.
pub fn unarchive_node(root: &Path, graph: &mut Graph, id: &str) -> Result<()> {
    let archive_dir = root.join(ARCHIVE_DIR);

    // Try per-node archive file first, fall back to legacy JSONL
    let node = remove_from_archive_nodes(&archive_dir, id)?
        .or_else(|| remove_from_archive_jsonl(&archive_dir, id).ok().flatten())
        .with_context(|| format!("node '{}' not found in archive", id))?;

    // Restore outgoing edges into the graph
    let live_ids: std::collections::HashSet<&str> =
        graph.nodes.iter().map(|n| n.id.as_str()).collect();
    for oe in &node.outgoing_edges {
        // Only restore edges where target is live (or is this node itself)
        if live_ids.contains(oe.to.as_str()) || oe.to == id {
            graph.edges.push(Edge {
                from: id.to_string(),
                to: oe.to.clone(),
                edge_type: oe.edge_type.clone(),
            });
        }
    }

    // Restore dormant edges from live nodes that point to this node
    for live_node_path in fs::read_dir(root.join(NODES_DIR))? {
        let live_node_path = live_node_path?.path();
        if live_node_path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&live_node_path)?;
        let v: serde_json::Value = serde_json::from_str(&raw)?;
        let from_id = v["id"].as_str().unwrap_or_default();
        if let Some(edges) = v["edges"].as_array() {
            for e in edges {
                if e["to"].as_str() == Some(id) {
                    let edge_type: crate::model::EdgeType =
                        serde_json::from_value(e["type"].clone())?;
                    // Only add if not already in graph.edges
                    if !graph.edges.iter().any(|ge|
                        ge.from == from_id && ge.to == id && ge.edge_type == edge_type
                    ) {
                        graph.edges.push(Edge {
                            from: from_id.to_string(),
                            to: id.to_string(),
                            edge_type,
                        });
                    }
                }
            }
        }
    }

    // Move body back to issues/
    let src = archive_dir.join(format!("{}.md", id));
    let dst = root.join(ISSUES_DIR).join(format!("{}.md", id));
    if src.exists() {
        fs::rename(src, dst)?;
    }

    // Move comments back to issues/
    let csrc = archive_dir.join(format!("{}.comments.json", id));
    let cdst = root.join(ISSUES_DIR).join(format!("{}.comments.json", id));
    if csrc.exists() {
        fs::rename(csrc, cdst)?;
    }

    graph.nodes.push(node);

    Ok(())
}

/// Remove a node from archive/nodes/{id}.json, returning it.
fn remove_from_archive_nodes(archive_dir: &Path, id: &str) -> Result<Option<Node>> {
    let path = archive_dir.join(ARCHIVE_NODES_DIR).join(format!("{}.json", id));
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)?;
    let node: Node = serde_json::from_str(&json)?;
    fs::remove_file(&path)?;
    Ok(Some(node))
}

/// Remove any edges pointing to the given node from all node files (live + archived).
/// Called by cx rm to prevent orphaned edges.
pub fn scrub_archived_edges(root: &Path, id: &str) -> Result<()> {
    // Scrub from live node files
    scrub_edges_in_dir(&root.join(NODES_DIR), id)?;
    // Scrub from archived node files
    scrub_edges_in_dir(&root.join(ARCHIVE_DIR).join(ARCHIVE_NODES_DIR), id)?;
    Ok(())
}

/// Remove edges pointing to `id` from all node JSON files in `dir`.
fn scrub_edges_in_dir(dir: &Path, id: &str) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path)?;
        let mut v: serde_json::Value = serde_json::from_str(&raw)?;
        if let Some(edges) = v["edges"].as_array() {
            let filtered: Vec<_> = edges.iter()
                .filter(|e| e["to"].as_str() != Some(id))
                .cloned()
                .collect();
            if filtered.len() != edges.len() {
                v["edges"] = serde_json::Value::Array(filtered);
                fs::write(&path, serde_json::to_string_pretty(&v)?)?;
            }
        }
    }
    Ok(())
}

/// Migrate a legacy archive.json or archive.jsonl to per-node archive files.
pub fn migrate_archive_if_needed(root: &Path) -> Result<()> {
    // Migrate archive.json → per-node files
    let legacy_json = root.join(ARCHIVE_DIR).join("archive.json");
    if legacy_json.exists() {
        let raw = fs::read_to_string(&legacy_json)?;
        let nodes: Vec<Node> = serde_json::from_str(&raw).unwrap_or_default();
        let archive_nodes = root.join(ARCHIVE_DIR).join(ARCHIVE_NODES_DIR);
        fs::create_dir_all(&archive_nodes)?;
        for node in &nodes {
            let json = serde_json::to_string_pretty(node)?;
            fs::write(archive_nodes.join(format!("{}.json", node.id)), json)?;
        }
        fs::remove_file(&legacy_json)?;
    }

    // Migrate archive.jsonl → per-node files
    let archive_dir = root.join(ARCHIVE_DIR);
    if archive_dir.exists() {
        let archive_nodes = archive_dir.join(ARCHIVE_NODES_DIR);
        for entry in fs::read_dir(&archive_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let content = fs::read_to_string(&path)?;
            let mut migrated_any = false;
            for line in content.lines().filter(|l| !l.trim().is_empty()) {
                if let Ok(node) = serde_json::from_str::<Node>(line) {
                    fs::create_dir_all(&archive_nodes)?;
                    let json = serde_json::to_string_pretty(&node)?;
                    fs::write(archive_nodes.join(format!("{}.json", node.id)), json)?;
                    migrated_any = true;
                }
            }
            if migrated_any {
                fs::remove_file(&path)?;
            }
        }
    }

    // Migrate edges.jsonl → absorb into source node files, then delete
    let edges_path = root.join(ARCHIVE_DIR).join("edges.jsonl");
    if edges_path.exists() {
        let content = fs::read_to_string(&edges_path)?;
        let nodes_dir = root.join(NODES_DIR);
        let archive_nodes = root.join(ARCHIVE_DIR).join(ARCHIVE_NODES_DIR);
        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            let edge: serde_json::Value = serde_json::from_str(line)?;
            let from = edge["from"].as_str().unwrap_or_default();
            let to = edge["to"].as_str().unwrap_or_default();
            let etype = &edge["type"];
            // Find the source node file (live or archived)
            let node_path = [
                nodes_dir.join(format!("{}.json", from)),
                archive_nodes.join(format!("{}.json", from)),
            ]
            .into_iter()
            .find(|p| p.exists());
            if let Some(path) = node_path {
                let raw = fs::read_to_string(&path)?;
                let mut v: serde_json::Value = serde_json::from_str(&raw)?;
                let edges = v.get_mut("edges")
                    .and_then(|e| e.as_array_mut());
                let new_edge = serde_json::json!({"to": to, "type": etype});
                if let Some(arr) = edges {
                    if !arr.iter().any(|e| e["to"].as_str() == Some(to) && e["type"] == *etype) {
                        arr.push(new_edge);
                        fs::write(&path, serde_json::to_string_pretty(&v)?)?;
                    }
                } else {
                    v["edges"] = serde_json::json!([new_edge]);
                    fs::write(&path, serde_json::to_string_pretty(&v)?)?;
                }
            }
            // If no node file found, edge is orphaned — drop it
        }
        fs::remove_file(&edges_path)?;
    }

    Ok(())
}

/// Find a node by ID in legacy archive JSONL files, remove its line, return the node.
fn remove_from_archive_jsonl(archive_dir: &Path, id: &str) -> Result<Option<Node>> {
    if !archive_dir.exists() {
        return Ok(None);
    }
    for entry in fs::read_dir(archive_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        let mut found = None;
        let mut remaining = Vec::new();
        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            if found.is_none()
                && let Ok(node) = serde_json::from_str::<Node>(line)
                && node.id == id
            {
                found = Some(node);
                continue;
            }
            remaining.push(line.to_string());
        }
        if let Some(node) = found {
            if remaining.is_empty() {
                fs::remove_file(&path)?;
            } else {
                fs::write(&path, remaining.join("\n") + "\n")?;
            }
            return Ok(Some(node));
        }
    }
    Ok(None)
}



/// Collect all IDs from archived nodes.
pub fn load_archived_ids(root: &Path) -> Result<std::collections::HashSet<String>> {
    let mut ids = std::collections::HashSet::new();

    // Per-node archive files
    let archive_nodes = root.join(ARCHIVE_DIR).join(ARCHIVE_NODES_DIR);
    if archive_nodes.exists() {
        for entry in fs::read_dir(&archive_nodes)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                && path.extension().and_then(|e| e.to_str()) == Some("json")
            {
                ids.insert(stem.to_string());
            }
        }
    }

    // Legacy: scan JSONL files for any remaining archived nodes
    let archive_dir = root.join(ARCHIVE_DIR);
    if archive_dir.exists() {
        for entry in fs::read_dir(&archive_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                let content = fs::read_to_string(&path)?;
                for line in content.lines() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line)
                        && let Some(id) = v["id"].as_str()
                    {
                        ids.insert(id.to_string());
                    }
                }
            }
        }
    }
    Ok(ids)
}

// ── orphan detection ──────────────────────────────────────────────────────

/// Find .md files in issues/ that don't correspond to any node in the graph.
pub fn find_orphan_bodies(root: &Path, graph: &Graph) -> Result<Vec<String>> {
    let issues = root.join(ISSUES_DIR);
    if !issues.exists() {
        return Ok(vec![]);
    }
    let mut orphans = vec![];
    for entry in fs::read_dir(&issues)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".md")
            && graph.get_node(id).is_none()
        {
            orphans.push(id.to_string());
        }
    }
    orphans.sort();
    Ok(orphans)
}

