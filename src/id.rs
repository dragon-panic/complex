use std::collections::HashSet;

use anyhow::{bail, Result};
use crate::model::Graph;

const B62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const MAX_RETRIES: usize = 10;

pub fn generate(parent: Option<&str>, existing: &HashSet<String>) -> Result<String> {
    for _ in 0..MAX_RETRIES {
        let seg: String = (0..4)
            .map(|_| B62[rand::random::<usize>() % 62] as char)
            .collect();
        let full = match parent {
            Some(p) => format!("{}.{}", p, seg),
            None => seg,
        };
        if !existing.contains(&full) {
            return Ok(full);
        }
    }
    bail!("failed to generate unique ID after {} attempts", MAX_RETRIES)
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
