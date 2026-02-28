use anyhow::{bail, Result};
use crate::model::Graph;

const B62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

pub fn generate(parent: Option<&str>) -> String {
    let seg: String = (0..4)
        .map(|_| B62[rand::random::<usize>() % 62] as char)
        .collect();
    match parent {
        Some(p) => format!("{}.{}", p, seg),
        None => seg,
    }
}

pub fn parent_of(id: &str) -> Option<&str> {
    id.rfind('.').map(|i| &id[..i])
}

pub fn depth(id: &str) -> usize {
    id.chars().filter(|&c| c == '.').count()
}

/// Resolve a possibly-short id (leaf segment) to a full id.
/// Exact match wins. Otherwise matches any node whose id ends with ".<partial>".
pub fn resolve(graph: &Graph, partial: &str) -> Result<String> {
    if graph.get_node(partial).is_some() {
        return Ok(partial.to_string());
    }
    let suffix = format!(".{}", partial);
    let matches: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.id.ends_with(&suffix))
        .map(|n| n.id.as_str())
        .collect();
    match matches.len() {
        0 => bail!("no node matching '{}'", partial),
        1 => Ok(matches[0].to_string()),
        _ => bail!(
            "ambiguous id '{}' — matches: {}",
            partial,
            matches.join(", ")
        ),
    }
}
