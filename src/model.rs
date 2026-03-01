use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Latent,
    Ready,
    Claimed,
    Integrated,
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            State::Latent => "latent",
            State::Ready => "ready",
            State::Claimed => "claimed",
            State::Integrated => "integrated",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeType {
    Blocks,
    WaitsFor,
    DiscoveredFrom,
    Related,
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EdgeType::Blocks => "blocks",
            EdgeType::WaitsFor => "waits-for",
            EdgeType::DiscoveredFrom => "discovered-from",
            EdgeType::Related => "related",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub title: String,
    pub state: State,
    #[serde(default)]
    pub shadowed: bool,
    #[serde(default)]
    pub part: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Arbitrary JSON for orchestrators, agents, and workflow engines.
    /// complex stores it and ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    #[serde(skip)]
    pub body: Option<String>,
}

impl Node {
    pub fn new(id: String, title: String) -> Self {
        let now = Utc::now();
        Node {
            id,
            title,
            state: State::Latent,
            shadowed: false,
            part: None,
            created_at: now,
            updated_at: now,
            meta: None,
            body: None,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub edge_type: EdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub version: u32,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Default for Graph {
    fn default() -> Self {
        Graph {
            version: 1,
            nodes: vec![],
            edges: vec![],
        }
    }
}

impl Graph {
    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Direct children of a node (one level deep only).
    pub fn children(&self, parent_id: &str) -> Vec<&Node> {
        let prefix = format!("{}.", parent_id);
        self.nodes
            .iter()
            .filter(|n| n.id.starts_with(&prefix) && !n.id[prefix.len()..].contains('.'))
            .collect()
    }

    /// All root nodes (no dot in id).
    pub fn roots(&self) -> Vec<&Node> {
        self.nodes.iter().filter(|n| !n.id.contains('.')).collect()
    }

    /// Returns true if adding a blocks edge from `from` to `to` would create a cycle.
    /// Uses DFS: a cycle exists if `from` is reachable from `to` via existing blocks edges.
    pub fn would_cycle(&self, from: &str, to: &str) -> bool {
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![to];
        while let Some(node) = stack.pop() {
            if node == from {
                return true;
            }
            if visited.insert(node) {
                for e in &self.edges {
                    if e.from == node && e.edge_type == EdgeType::Blocks {
                        stack.push(&e.to);
                    }
                }
            }
        }
        false
    }
}
