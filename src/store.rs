use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::model::{Graph, Node};

const COMPLEX_DIR: &str = ".complex";
const GRAPH_FILE: &str = "graph.json";
const ISSUES_DIR: &str = "issues";
const ARCHIVE_DIR: &str = "archive";
const ARCHIVE_GRAPH: &str = "archive.json";

pub fn find_root() -> Result<PathBuf> {
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

pub fn init(cwd: &Path) -> Result<()> {
    let root = cwd.join(COMPLEX_DIR);
    if root.exists() {
        bail!(".complex/ already exists here");
    }
    fs::create_dir_all(root.join(ISSUES_DIR))?;
    fs::create_dir_all(root.join(ARCHIVE_DIR))?;
    let json = serde_json::to_string_pretty(&Graph::default())?;
    fs::write(root.join(GRAPH_FILE), json)?;
    Ok(())
}

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
    }

    Ok(graph)
}

pub fn save(root: &Path, graph: &Graph) -> Result<()> {
    // Atomic write for graph.json
    let graph_path = root.join(GRAPH_FILE);
    let tmp = root.join("graph.json.tmp");
    let json = serde_json::to_string_pretty(graph)?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &graph_path)?;

    // Write bodies that are present in memory
    let issues = root.join(ISSUES_DIR);
    for node in &graph.nodes {
        if let Some(body) = &node.body {
            fs::write(issues.join(format!("{}.md", node.id)), body)?;
        }
    }

    Ok(())
}

pub fn write_body(root: &Path, id: &str, body: &str) -> Result<()> {
    let path = root.join(ISSUES_DIR).join(format!("{}.md", id));
    fs::write(path, body)?;
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

pub fn archive_node(root: &Path, graph: &mut Graph, id: &str) -> Result<()> {
    let pos = graph
        .nodes
        .iter()
        .position(|n| n.id == id)
        .with_context(|| format!("node '{}' not found", id))?;
    let node = graph.nodes.remove(pos);

    // Move markdown file
    let src = root.join(ISSUES_DIR).join(format!("{}.md", id));
    let dst = root.join(ARCHIVE_DIR).join(format!("{}.md", id));
    if src.exists() {
        fs::rename(src, dst)?;
    }

    // Append to archive.json
    let archive_path = root.join(ARCHIVE_DIR).join(ARCHIVE_GRAPH);
    let mut archived: Vec<Node> = if archive_path.exists() {
        let raw = fs::read_to_string(&archive_path)?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        vec![]
    };
    archived.push(node);
    fs::write(archive_path, serde_json::to_string_pretty(&archived)?)?;

    Ok(())
}
