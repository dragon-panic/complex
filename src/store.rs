use std::fs::{self, File, OpenOptions};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use fs2::FileExt;

use crate::model::{Graph, Node};

const COMPLEX_DIR: &str = ".complex";
const GRAPH_FILE: &str = "graph.json";
const ISSUES_DIR: &str = "issues";
const ARCHIVE_DIR: &str = "archive";
const ARCHIVE_JSONL: &str = "archive.jsonl";
const EVENTS_JSONL: &str = "events.jsonl";
const EVENTS_DIR: &str = "events";
const AGENTS_FILE: &str = "agents.json";

/// Lines before rotating the active archive file.
const ARCHIVE_ROTATE_LINES: usize = 200;
/// Lines before rotating the active events file.
const EVENTS_ROTATE_LINES: usize = 1000;

// ── project location ──────────────────────────────────────────────────────────

pub fn find_root() -> Result<PathBuf> {
    if let Ok(cx_dir) = std::env::var("CX_DIR") {
        let p = PathBuf::from(cx_dir);
        if p.join(GRAPH_FILE).exists() {
            return Ok(p);
        }
        bail!("CX_DIR is set to {} but no graph.json found there — run cx init", p.display());
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
    fs::create_dir_all(root.join(ISSUES_DIR))?;
    fs::create_dir_all(root.join(ARCHIVE_DIR))?;
    fs::create_dir_all(root.join(EVENTS_DIR))?;
    let json = serde_json::to_string_pretty(&Graph::default())?;
    fs::write(root.join(GRAPH_FILE), json)?;
    Ok(root)
}

// ── graph load / save ─────────────────────────────────────────────────────────

pub fn load(root: &Path) -> Result<Graph> {
    let path = root.join(GRAPH_FILE);
    let json = fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let mut graph: Graph = serde_json::from_str(&json)?;

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

    Ok(graph)
}

pub fn save(root: &Path, graph: &Graph) -> Result<()> {
    let lock_path = root.join("graph.lock");
    let lock_file = File::create(&lock_path)?;
    lock_file.lock_exclusive().context("acquiring graph.lock")?;

    let graph_path = root.join(GRAPH_FILE);
    let tmp = root.join("graph.json.tmp");
    let json = serde_json::to_string_pretty(graph)?;
    fs::write(&tmp, &json)?;
    fs::rename(&tmp, &graph_path)?;

    let issues = root.join(ISSUES_DIR);
    for node in &graph.nodes {
        if let Some(body) = &node.body {
            fs::write(issues.join(format!("{}.md", node.id)), body)?;
        }
        let comments_path = issues.join(format!("{}.comments.json", node.id));
        if node.comments.is_empty() {
            // Clean up file if all comments removed
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

/// Append a single node to archive.jsonl, rotating if needed.
pub fn archive_node(root: &Path, graph: &mut Graph, id: &str) -> Result<()> {
    let pos = graph
        .nodes
        .iter()
        .position(|n| n.id == id)
        .with_context(|| format!("node '{}' not found", id))?;
    let node = graph.nodes.remove(pos);

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

    // Append node as a single JSONL line, rotating first if needed
    let archive_path = root.join(ARCHIVE_DIR).join(ARCHIVE_JSONL);
    maybe_rotate(&archive_path, &root.join(ARCHIVE_DIR), ARCHIVE_ROTATE_LINES)?;

    let line = serde_json::to_string(&node)?;
    append_line(&archive_path, &line)?;

    // Remove edges that referenced the archived node
    graph.edges.retain(|e| e.from != id && e.to != id);

    Ok(())
}

/// Migrate a legacy archive.json to archive.jsonl in place.
/// Called transparently on first archive write.
pub fn migrate_archive_if_needed(root: &Path) -> Result<()> {
    let legacy = root.join(ARCHIVE_DIR).join("archive.json");
    if !legacy.exists() {
        return Ok(());
    }
    let raw = fs::read_to_string(&legacy)?;
    let nodes: Vec<Node> = serde_json::from_str(&raw).unwrap_or_default();
    if nodes.is_empty() {
        fs::remove_file(&legacy)?;
        return Ok(());
    }
    let dest = root.join(ARCHIVE_DIR).join(ARCHIVE_JSONL);
    let mut file = OpenOptions::new().create(true).append(true).open(&dest)?;
    for node in &nodes {
        writeln!(file, "{}", serde_json::to_string(node)?)?;
    }
    fs::remove_file(legacy)?;
    Ok(())
}

// ── events ────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct Event<'a> {
    pub ts: String,
    pub action: &'a str,
    pub node_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'a str>,
}

pub fn append_event(root: &Path, event: Event<'_>) -> Result<()> {
    fs::create_dir_all(root.join(EVENTS_DIR))?;
    let events_path = root.join(EVENTS_JSONL);
    maybe_rotate(&events_path, &root.join(EVENTS_DIR), EVENTS_ROTATE_LINES)?;
    let line = serde_json::to_string(&event)?;
    append_line(&events_path, &line)?;
    Ok(())
}

/// Read the current events.jsonl (most recent N lines).
pub fn recent_events(root: &Path, limit: usize) -> Result<Vec<serde_json::Value>> {
    let path = root.join(EVENTS_JSONL);
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(&path)?;
    let all: Vec<serde_json::Value> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let skip = all.len().saturating_sub(limit);
    Ok(all.into_iter().skip(skip).collect())
}

// ── agent registry ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentEntry {
    pub name: String,
    pub last_seen: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

pub fn upsert_agent(root: &Path, name: &str) -> Result<()> {
    let mut agents = load_agents(root)?;
    let ts = Utc::now().to_rfc3339();
    if let Some(entry) = agents.iter_mut().find(|a| a.name == name) {
        entry.last_seen = ts;
    } else {
        agents.push(AgentEntry {
            name: name.to_string(),
            last_seen: ts,
            meta: None,
        });
    }
    save_agents(root, &agents)
}

pub fn load_agents(root: &Path) -> Result<Vec<AgentEntry>> {
    let path = root.join(AGENTS_FILE);
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn save_agents(root: &Path, agents: &[AgentEntry]) -> Result<()> {
    fs::write(
        root.join(AGENTS_FILE),
        serde_json::to_string_pretty(agents)?,
    )?;
    Ok(())
}

/// Collect all IDs from archived nodes (active + rotated JSONL files).
pub fn load_archived_ids(root: &Path) -> Result<std::collections::HashSet<String>> {
    let archive_dir = root.join(ARCHIVE_DIR);
    let mut ids = std::collections::HashSet::new();
    if !archive_dir.exists() {
        return Ok(ids);
    }
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

// ── rotation ──────────────────────────────────────────────────────────────────

/// Rotate `active` into `archive_dir/YYYY-MM[-N].jsonl` if it exceeds
/// `max_lines` or the first entry is from a previous calendar month.
fn maybe_rotate(active: &Path, archive_dir: &Path, max_lines: usize) -> Result<()> {
    if !active.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(active)?;
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    if lines.is_empty() {
        return Ok(());
    }

    let now = Utc::now();
    let current_month = now.format("%Y-%m").to_string();

    let first_month = lines
        .first()
        .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .and_then(|v| v["ts"].as_str().map(|s| s[..7].to_string()));

    let needs_rotation = lines.len() >= max_lines
        || first_month.as_deref().map(|m| m != current_month).unwrap_or(false);

    if !needs_rotation {
        return Ok(());
    }

    let month_label = first_month.as_deref().unwrap_or(&current_month);
    let dest = unique_archive_path(archive_dir, month_label);
    fs::rename(active, dest)?;

    Ok(())
}

/// Returns a non-colliding path like `archive_dir/2026-02.jsonl`,
/// falling back to `2026-02-2.jsonl`, `2026-02-3.jsonl` etc.
fn unique_archive_path(dir: &Path, month: &str) -> PathBuf {
    let base = dir.join(format!("{}.jsonl", month));
    if !base.exists() {
        return base;
    }
    let mut n = 2usize;
    loop {
        let p = dir.join(format!("{}-{}.jsonl", month, n));
        if !p.exists() {
            return p;
        }
        n += 1;
    }
}

fn append_line(path: &Path, line: &str) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}
