use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub timestamp: DateTime<Utc>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    pub body: String,
}

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
    /// Who/what created this node (e.g. "ox", "claude@seguro").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filed_by: Option<String>,
    /// Tags for categorization and filtering. Propagated to children at read time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Explicit parent node ID. `None` = root node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Arbitrary JSON for orchestrators, agents, and workflow engines.
    /// complex stores it and ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    /// Outgoing edges stored in per-node files. Expanded into Graph.edges on load.
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "edges")]
    pub outgoing_edges: Vec<OutgoingEdge>,
    #[serde(skip)]
    pub body: Option<String>,
    #[serde(skip)]
    pub comments: Vec<Comment>,
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
            filed_by: None,
            tags: vec![],
            parent: None,
            created_at: now,
            updated_at: now,
            meta: None,
            outgoing_edges: vec![],
            body: None,
            comments: vec![],
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

/// Outgoing edge stored in a per-node file. `from` is implicit (the node's own ID).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingEdge {
    pub to: String,
    #[serde(rename = "type")]
    pub edge_type: EdgeType,
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
        self.nodes
            .iter()
            .filter(|n| n.parent.as_deref() == Some(parent_id))
            .collect()
    }

    /// All root nodes (no parent).
    pub fn roots(&self) -> Vec<&Node> {
        self.nodes.iter().filter(|n| n.parent.is_none()).collect()
    }

    /// All ancestor IDs from immediate parent up to root.
    pub fn ancestors(&self, id: &str) -> Vec<String> {
        let mut result = vec![];
        let mut cur = id;
        while let Some(node) = self.get_node(cur) {
            if let Some(p) = &node.parent {
                result.push(p.clone());
                cur = p;
            } else {
                break;
            }
        }
        result
    }

    /// All transitive descendants (BFS).
    #[allow(dead_code)]
    pub fn descendants(&self, id: &str) -> Vec<&Node> {
        let mut result = vec![];
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(id.to_string());
        while let Some(cur) = queue.pop_front() {
            for child in self.children(&cur) {
                result.push(child);
                queue.push_back(child.id.clone());
            }
        }
        result
    }

    /// Returns true if `candidate` is a descendant of `ancestor`.
    pub fn is_descendant_of(&self, candidate: &str, ancestor: &str) -> bool {
        let mut cur = candidate;
        while let Some(node) = self.get_node(cur) {
            if let Some(p) = &node.parent {
                if p == ancestor {
                    return true;
                }
                cur = p;
            } else {
                break;
            }
        }
        false
    }

    /// Returns IDs of latent nodes that have no non-integrated blockers.
    /// These are candidates for promotion to ready.
    pub fn unblocked_latent_ids(&self) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|n| n.state == State::Latent)
            .filter(|n| {
                !self.edges.iter().any(|e| {
                    e.to == n.id
                        && e.edge_type == EdgeType::Blocks
                        && self
                            .get_node(&e.from)
                            .is_some_and(|b| b.state != State::Integrated)
                })
            })
            .map(|n| n.id.clone())
            .collect()
    }

    /// Compute effective tags for a node: own tags + all ancestor tags (deduplicated).
    pub fn effective_tags(&self, id: &str) -> Vec<String> {
        let mut tags = std::collections::BTreeSet::new();
        if let Some(node) = self.get_node(id) {
            tags.extend(node.tags.iter().cloned());
        }
        for ancestor_id in self.ancestors(id) {
            if let Some(ancestor) = self.get_node(&ancestor_id) {
                tags.extend(ancestor.tags.iter().cloned());
            }
        }
        tags.into_iter().collect()
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
