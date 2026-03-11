mod db;
mod id;
mod model;
mod store;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use model::{EdgeType, State};

#[derive(Parser)]
#[command(name = "cx", about = "complex — hierarchical issue tracker for agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize .complex/ in the current directory
    Init {
        /// Add .complex/ to .gitignore (for scratch/CI/agent use)
        #[arg(long)]
        ephemeral: bool,
    },

    /// Create a new root complex (human-facing entry point)
    Add {
        title: Vec<String>,
        /// Set body inline, or pipe stdin (e.g. echo "md" | cx add "title" --body -)
        #[arg(long, conflicts_with = "body_file")]
        body: Option<String>,
        /// Read body from a file path
        #[arg(long = "body-file", short = 'F', conflicts_with = "body")]
        body_file: Option<String>,
        /// Who/what is filing this (falls back to CX_FILED_BY env var)
        #[arg(long)]
        by: Option<String>,
        /// Add tags to the node
        #[arg(long, value_name = "TAG")]
        tag: Vec<String>,
    },

    /// Create a child node under a parent
    New {
        parent_id: String,
        title: Vec<String>,
        /// Set body inline, or pipe stdin (e.g. echo "md" | cx new <parent> "title" --body -)
        #[arg(long, conflicts_with = "body_file")]
        body: Option<String>,
        /// Read body from a file path
        #[arg(long = "body-file", short = 'F', conflicts_with = "body")]
        body_file: Option<String>,
        /// Who/what is filing this (falls back to CX_FILED_BY env var)
        #[arg(long)]
        by: Option<String>,
        /// Add tags to the node
        #[arg(long, value_name = "TAG")]
        tag: Vec<String>,
    },

    /// List ready nodes, or promote latent nodes to ready
    Surface {
        ids: Vec<String>,
        /// Why these nodes are being surfaced
        #[arg(long)]
        reason: Option<String>,
        /// Promote ALL latent nodes with no open blockers to ready in one shot
        #[arg(long, conflicts_with = "ids")]
        all: bool,
    },

    /// Claim a node for a part (agent)
    Claim {
        id: String,
        #[arg(long, value_name = "PART")]
        r#as: Option<String>,
        /// Why you are claiming this task
        #[arg(long)]
        reason: Option<String>,
    },

    /// Release a claim, returning the node to ready
    Unclaim {
        id: String,
        /// Why you are releasing this claim
        #[arg(long)]
        reason: Option<String>,
    },

    /// Mark a node as integrated (done) and move it to archive
    Integrate {
        id: String,
        /// Outcome or rationale for integration
        #[arg(long)]
        reason: Option<String>,
    },

    /// List shadowed nodes, or flag a node as shadowed
    Shadow {
        id: Option<String>,
        /// Why this node is being shadowed
        #[arg(long)]
        reason: Option<String>,
    },

    /// Clear the shadow flag from a node
    Unshadow {
        id: String,
        /// Why this node is being unshadowed
        #[arg(long)]
        reason: Option<String>,
    },

    /// Show node detail
    Show { id: String },

    /// Show the full hierarchy with states
    Tree {
        id: Option<String>,
        /// Filter by effective tag (own + inherited)
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,
    },

    /// Show claimed nodes grouped by part
    Parts,

    /// Show stale or stuck nodes needing review
    Therapy,

    /// Remove a node (discard, not integrate)
    Rm {
        id: String,
        /// Why this node is being removed
        #[arg(long)]
        reason: Option<String>,
    },

    /// Rename a node's title
    Rename {
        id: String,
        title: Vec<String>,
    },

    /// Set a node's body (auto-detects: piped → stdin, TTY → $EDITOR)
    Edit {
        id: String,
        /// Set body directly from this string
        #[arg(long, conflicts_with_all = ["file", "editor"])]
        body: Option<String>,
        /// Read body from a file path
        #[arg(long, conflicts_with_all = ["body", "editor"])]
        file: Option<String>,
        /// Force $EDITOR even when stdin is piped
        #[arg(long, conflicts_with_all = ["body", "file"])]
        editor: bool,
    },

    /// Move a node (and its children) under a new parent
    #[command(name = "move", alias = "mv")]
    Move {
        id: String,
        /// New parent node ID (omit for --root)
        new_parent: Option<String>,
        /// Promote to a root node (no parent)
        #[arg(long)]
        root: bool,
        /// Why this node is being moved
        #[arg(long)]
        reason: Option<String>,
    },

    /// Declare that node A blocks node B
    Block { a: String, b: String },

    /// Remove a blocks edge
    Unblock { a: String, b: String },

    /// Add a related (non-blocking) edge between two nodes
    Relate { a: String, b: String },

    /// Mark node A as discovered while working on node B
    Discover { a: String, b: String },

    /// Add a tag to a node
    Tag {
        id: String,
        tag: String,
    },

    /// Remove a tag from a node
    Untag {
        id: String,
        tag: String,
    },

    /// Show effective tags for a node, or list all tags in use
    Tags {
        id: Option<String>,
    },

    /// Search nodes by title (case-insensitive substring match)
    Find {
        query: Vec<String>,
        /// Filter by effective tag (own + inherited)
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,
    },

    /// List all nodes, optionally filtered by state or filer
    List {
        #[arg(long, value_name = "STATE")]
        state: Option<String>,
        /// Filter by who filed the node
        #[arg(long, value_name = "WHO")]
        filed_by: Option<String>,
        /// Filter by effective tag (own + inherited)
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,
    },

    /// Read or write arbitrary metadata on a node (JSON blob)
    Meta {
        id: String,
        /// JSON value to set. Omit to read current metadata.
        value: Option<String>,
    },

    /// Print the agent guide (pipe to AGENT.md or include in system prompt)
    Agent,

    /// Show recent events (audit log)
    Log {
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Show registered agents and their last-seen time
    Agents,

    /// Show tree + ready nodes (quick overview)
    Status,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init { ephemeral } => cmd_init(ephemeral),
        Commands::Add { title, body, body_file, by, tag } => cmd_add(title.join(" "), body, body_file, by, tag, cli.json),
        Commands::New { parent_id, title, body, body_file, by, tag } => cmd_new(parent_id, title.join(" "), body, body_file, by, tag, cli.json),
        Commands::Surface { ids, reason, all } => cmd_surface(ids, reason, all, cli.json),
        Commands::Claim { id, r#as, reason } => cmd_claim(id, r#as, reason, cli.json),
        Commands::Unclaim { id, reason } => cmd_unclaim(id, reason, cli.json),
        Commands::Integrate { id, reason } => cmd_integrate(id, reason, cli.json),
        Commands::Shadow { id, reason } => cmd_shadow(id, reason, cli.json),
        Commands::Unshadow { id, reason } => cmd_unshadow(id, reason, cli.json),
        Commands::Show { id } => cmd_show(id, cli.json),
        Commands::Tag { id, tag } => cmd_tag(id, tag, cli.json),
        Commands::Untag { id, tag } => cmd_untag(id, tag, cli.json),
        Commands::Tags { id } => cmd_tags(id, cli.json),
        Commands::Tree { id, tag } => cmd_tree(id, tag, cli.json),
        Commands::Parts => cmd_parts(cli.json),
        Commands::Therapy => cmd_therapy(cli.json),
        Commands::Move { id, new_parent, root, reason } => cmd_move(id, new_parent, root, reason, cli.json),
        Commands::Rm { id, reason } => cmd_rm(id, reason, cli.json),
        Commands::Rename { id, title } => cmd_rename(id, title.join(" "), cli.json),
        Commands::Edit { id, body, file, editor } => cmd_edit(id, body, file, editor),
        Commands::Block { a, b } => cmd_edge(a, b, EdgeType::Blocks, false, cli.json),
        Commands::Unblock { a, b } => cmd_edge(a, b, EdgeType::Blocks, true, cli.json),
        Commands::Relate { a, b } => cmd_edge(a, b, EdgeType::Related, false, cli.json),
        Commands::Discover { a, b } => cmd_edge(a, b, EdgeType::DiscoveredFrom, false, cli.json),
        Commands::Find { query, tag } => cmd_find(query.join(" "), tag, cli.json),
        Commands::List { state, filed_by, tag } => cmd_list(state, filed_by, tag, cli.json),
        Commands::Agent => { print!("{}", AGENT_GUIDE); Ok(()) },
        Commands::Meta { id, value } => cmd_meta(id, value, cli.json),
        Commands::Log { limit } => cmd_log(limit, cli.json),
        Commands::Agents => cmd_agents(cli.json),
        Commands::Status => cmd_status(cli.json),
    }
}

// ── body helper ──────────────────────────────────────────────────────────────

/// Resolve a --body flag value: "-" means read stdin, anything else is literal.
fn resolve_body(raw: &str) -> Result<String> {
    if raw == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        Ok(raw.to_string())
    }
}

// ── event helper ─────────────────────────────────────────────────────────────

fn emit(root: &std::path::Path, action: &str, node_id: &str, part: Option<&str>, detail: Option<&str>, reason: Option<&str>) {
    let _ = store::append_event(root, store::Event {
        ts: chrono::Utc::now().to_rfc3339(),
        action,
        node_id,
        part,
        detail,
        reason,
    });
}

/// Merge `{"_reason": reason}` into a node's existing meta (or create it).
fn set_reason(node: &mut model::Node, reason: &str) {
    let meta = node.meta.get_or_insert_with(|| serde_json::json!({}));
    if let Some(obj) = meta.as_object_mut() {
        obj.insert("_reason".to_string(), serde_json::json!(reason));
    }
}

/// Returns true if a node or any descendant has the given effective tag.
fn subtree_has_tag(graph: &model::Graph, node_id: &str, tag: &str) -> bool {
    if graph.effective_tags(node_id).iter().any(|t| t == tag) {
        return true;
    }
    for child in graph.children(node_id) {
        if subtree_has_tag(graph, &child.id, tag) {
            return true;
        }
    }
    false
}

/// Returns the ID of the first ancestor (or self) that has an unresolved blocker,
/// or None if the node is unblocked.
fn ancestor_blocker(graph: &model::Graph, id: &str) -> Option<String> {
    // Collect self + all ancestor IDs from the dot-separated hierarchy
    let mut ancestors = vec![id.to_string()];
    let mut cur = id.to_string();
    while let Some(dot) = cur.rfind('.') {
        cur = cur[..dot].to_string();
        ancestors.push(cur.clone());
    }

    for ancestor in &ancestors {
        let has_blocker = graph.edges.iter().any(|e| {
            e.to == *ancestor
                && e.edge_type == model::EdgeType::Blocks
                && graph
                    .get_node(&e.from)
                    .map(|n| n.state != model::State::Integrated)
                    .unwrap_or(false)
        });
        if has_blocker {
            return Some(ancestor.clone());
        }
    }
    None
}

// ── agent guide ──────────────────────────────────────────────────────────────

const AGENT_GUIDE: &str = r#"# complex — agent guide

This project uses `complex` (cx) for task tracking. You are a **part** —
an agent that claims, works, and integrates tasks.

## Workflow

1. Find available work:   cx surface --json
2. Claim a task:          cx claim <id> --as <your-name>
3. Do the work
4. If you discover a sub-task while working:
                          cx new <parent-id> <title>
                          cx discover <new-id> <current-id>
5. Mark done:             cx integrate <id>
6. Commit:                git add .complex/ && git commit -m "integrate(<id>): <title>"

Tasks are **parallel by default**. Only an explicit `cx block <a> <b>` creates
ordering. Run `cx surface` at any time — it only shows tasks with no open blockers.

## Commands you will use

```
cx status --json                  tree + ready nodes (quick overview)
cx surface --json                 ready tasks (no open blockers)
cx surface --all --json           promote all latent tasks with no blockers to ready
cx claim <id> --as <name>         take ownership (or set CX_PART env var)
cx unclaim <id>                   release if you cannot complete it
cx integrate <id>                 mark done → archive; auto-surfaces any newly unblocked latent tasks
                                  JSON includes "newly_surfaced": [...] when tasks are unblocked
cx rm <id>                        remove/discard a node (not integrate)
cx new <parent-id> <title>        create a child task under a parent
cx add <title> --body "markdown"  create with body in one shot (also works on cx new)
cx add <title> --by <who>        record who filed this (or set CX_FILED_BY)
cx discover <new-id> <source-id>  record task found while working on source
cx find <query>                   search nodes by title (case-insensitive)
cx tag <id> <tag>                 add a tag to a node
cx untag <id> <tag>               remove a tag from a node
cx tags [id]                      show effective tags (own + inherited) or list all
cx rename <id> <new title>        rename a node
cx move <id> <new-parent>         reparent a node (and children) under a new parent
cx move <id> --root               promote a node to root level
cx shadow <id>                    flag as blocked/stuck
cx edit <id> --body "markdown"    update body non-interactively (or pipe: echo "md" | cx edit <id>)
cx show <id> --json               full node detail: state, edges, body, children
cx tree --json                    full hierarchy with states (nested children)
cx parts --json                   what each part currently holds
cx therapy --json                 stale (claimed >24h), shadowed, and orphan body files
cx list --state claimed --json    all nodes in a given state
```

## Rationale (--reason)

All mutation commands accept an optional `--reason` flag to record **why** you
are taking an action. The reason is stored in `events.jsonl` (audit trail) and
in the node's `meta._reason` field (quick lookup for orchestrators).

```
cx claim <id> --as agent-1 --reason "has rust capability, no blockers"
cx shadow <id> --reason "tests failing, needs upstream fix in auth module"
cx unclaim <id> --reason "blocked on external API, releasing for others"
cx integrate <id> --reason "all tests pass, code reviewed"
cx surface <id> --reason "dependency resolved, ready for work"
cx unshadow <id> --reason "upstream fix landed"
```

Reason is always optional — omitting it never blocks an action.

## State model

```
latent → ready → claimed → integrated
                    ↕
                 shadowed  (flag — orthogonal to state)
```

**Important:** `cx claim` only works on `ready` nodes. You must `cx surface <id>`
a latent node before claiming it.

## IDs

Hierarchy is encoded in the ID:
  a3F2              root complex
  a3F2.bX7c         child task
  a3F2.bX7c.Qd4e   grandchild subtask

Short IDs (leaf segment) work when unambiguous:  cx claim bX7c

## Environment

  CX_PART      your identity — set this before claiming anything
  CX_FILED_BY  default --by value (convention: project:agent, e.g. seguro:ox)

## What to commit

After any cx mutation, stage and commit `.complex/`:
  git add .complex/ && git commit -m "claim(bX7c): implement JWT tokens"
  git add .complex/ && git commit -m "integrate(bX7c): implement JWT tokens"
"#;

// ── init ─────────────────────────────────────────────────────────────────────

fn cmd_init(ephemeral: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = store::init(&cwd)?;
    println!("initialized {} in {}", root.display(), cwd.display());

    if ephemeral {
        // Only add to .gitignore if the root is inside cwd (i.e. not an external CX_DIR)
        if let Ok(rel) = root.strip_prefix(&cwd) {
            let gitignore = cwd.join(".gitignore");
            let entry_name = format!("{}/", rel.display());
            let needs_append = if gitignore.exists() {
                let content = std::fs::read_to_string(&gitignore)?;
                !content.lines().any(|l| {
                    let t = l.trim();
                    t == entry_name.trim_end_matches('/') || t == entry_name.trim()
                })
            } else {
                true
            };
            if needs_append {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&gitignore)?;
                f.write_all(entry_name.as_bytes())?;
                f.write_all(b"\n")?;
                println!("added {} to .gitignore", entry_name.trim());
            }
        } else {
            println!("--ephemeral ignored: {} is outside the project", root.display());
        }
    }
    Ok(())
}

// ── add / new ─────────────────────────────────────────────────────────────────

fn cmd_add(title: String, body: Option<String>, body_file: Option<String>, by: Option<String>, tags: Vec<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;

    let filed_by = by.or_else(|| std::env::var("CX_FILED_BY").ok());

    let new_id = id::generate(None);
    let mut node = model::Node::new(new_id.clone(), title.clone());
    node.filed_by = filed_by;
    node.tags = tags;
    graph.nodes.push(node);
    store::save(&root, &graph)?;

    if let Some(path) = &body_file {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {}", path, e))?;
        store::write_body(&root, &new_id, &content)?;
    } else if let Some(raw) = &body {
        let content = resolve_body(raw)?;
        store::write_body(&root, &new_id, &content)?;
    }

    emit(&root, "create", &new_id, None, Some(&title), None);

    if json {
        println!("{}", serde_json::json!({ "id": new_id, "title": title }));
    } else {
        println!("created  {}  {}", new_id, title);
    }
    Ok(())
}

fn cmd_new(parent_partial: String, title: String, body: Option<String>, body_file: Option<String>, by: Option<String>, tags: Vec<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;

    let filed_by = by.or_else(|| std::env::var("CX_FILED_BY").ok());

    let parent_id = id::resolve(&graph, &parent_partial)
        .map_err(|_| anyhow::anyhow!("parent '{}' not found — use cx tree to list nodes", parent_partial))?;
    let new_id = id::generate(Some(&parent_id));
    let mut node = model::Node::new(new_id.clone(), title.clone());
    node.filed_by = filed_by;
    node.tags = tags;
    graph.nodes.push(node);
    store::save(&root, &graph)?;

    if let Some(path) = &body_file {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {}", path, e))?;
        store::write_body(&root, &new_id, &content)?;
    } else if let Some(raw) = &body {
        let content = resolve_body(raw)?;
        store::write_body(&root, &new_id, &content)?;
    }

    emit(&root, "create", &new_id, None, Some(&title), None);

    if json {
        println!(
            "{}",
            serde_json::json!({ "id": new_id, "parent": parent_id, "title": title })
        );
    } else {
        println!("created  {}  {}  (child of {})", new_id, title, parent_id);
    }
    Ok(())
}

// ── surface ───────────────────────────────────────────────────────────────────

fn cmd_surface(ids: Vec<String>, reason: Option<String>, all: bool, json: bool) -> Result<()> {
    let root = store::find_root()?;

    if all {
        // Bulk-promote: all latent nodes with no open blockers → ready
        let mut graph = store::load(&root)?;
        let eligible = graph.unblocked_latent_ids();
        if eligible.is_empty() {
            if json {
                println!("[]");
            } else {
                println!("no latent nodes with open blockers cleared");
            }
            return Ok(());
        }
        for id in &eligible {
            if let Some(node) = graph.get_node_mut(id) {
                node.state = State::Ready;
                node.touch();
                if let Some(r) = &reason {
                    set_reason(node, r);
                }
            }
        }
        store::save(&root, &graph)?;
        for id in &eligible {
            emit(&root, "surface", id, None, None, reason.as_deref());
        }
        if json {
            let out: Vec<_> = eligible.iter().map(|id| serde_json::json!({ "id": id, "state": "ready" })).collect();
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            for id in &eligible {
                println!("surfaced  {}  → ready", id);
            }
        }
        return Ok(());
    }

    if ids.is_empty() {
        let graph = store::load(&root)?;
        let conn = db::materialize(&graph)?;
        let nodes = db::ready_nodes(&conn)?;

        if json {
            let out: Vec<_> = nodes
                .iter()
                .map(|n| serde_json::json!({ "id": n.id, "title": n.title, "part": n.part }))
                .collect();
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else if nodes.is_empty() {
            println!("no ready nodes");
        } else {
            for n in &nodes {
                let part = n.part.as_deref().unwrap_or("—");
                println!("{:<20}  {:<40}  {}", n.id, n.title, part);
            }
        }
    } else {
        let mut graph = store::load(&root)?;
        let mut results = Vec::new();

        for partial in &ids {
            let resolved = id::resolve(&graph, partial)?;
            let node = graph
                .get_node_mut(&resolved)
                .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;

            if node.state != State::Latent {
                bail!("{} is {} — only latent nodes can be surfaced", resolved, node.state);
            }
            node.state = State::Ready;
            node.touch();
            if let Some(r) = &reason {
                set_reason(node, r);
            }
            emit(&root, "surface", &resolved, None, None, reason.as_deref());
            results.push(resolved);
        }

        store::save(&root, &graph)?;

        if json {
            let out: Vec<_> = results.iter().map(|id| serde_json::json!({ "id": id, "state": "ready" })).collect();
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            for id in &results {
                println!("surfaced  {}  → ready", id);
            }
        }
    }
    Ok(())
}

// ── claim / unclaim ───────────────────────────────────────────────────────────

fn cmd_claim(partial: String, as_part: Option<String>, reason: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    let part = as_part
        .or_else(|| std::env::var("CX_PART").ok())
        .unwrap_or_else(|| "unknown".to_string());

    // Validate state with immutable borrow first
    {
        let node = graph
            .get_node(&resolved)
            .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;

        if node.state == State::Claimed {
            bail!(
                "{} is already claimed by {}",
                resolved,
                node.part.as_deref().unwrap_or("unknown")
            );
        }
        if node.state == State::Integrated {
            bail!("{} is already integrated", resolved);
        }
        if node.state == State::Latent {
            bail!("{} is latent — surface it first with: cx surface {}", resolved, resolved);
        }
    }

    // Check if this node or any ancestor is blocked
    if let Some(blocker) = ancestor_blocker(&graph, &resolved) {
        bail!("{} is blocked — ancestor {} has unresolved blockers", resolved, blocker);
    }

    let node = graph
        .get_node_mut(&resolved)
        .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;
    node.state = State::Claimed;
    node.part = Some(part.clone());
    node.touch();
    if let Some(r) = &reason {
        set_reason(node, r);
    }
    store::save(&root, &graph)?;
    store::upsert_agent(&root, &part).ok();
    emit(&root, "claim", &resolved, Some(&part), None, reason.as_deref());

    if json {
        println!("{}", serde_json::json!({ "id": resolved, "state": "claimed", "part": part }));
    } else {
        println!("claimed  {}  by {}", resolved, part);
    }
    Ok(())
}

fn cmd_unclaim(partial: String, reason: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    let node = graph
        .get_node_mut(&resolved)
        .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;

    if node.state != State::Claimed {
        bail!("{} is not claimed (state: {})", resolved, node.state);
    }

    node.state = State::Ready;
    node.part = None;
    node.touch();
    if let Some(r) = &reason {
        set_reason(node, r);
    }
    store::save(&root, &graph)?;
    emit(&root, "unclaim", &resolved, None, None, reason.as_deref());

    if json {
        println!("{}", serde_json::json!({ "id": resolved, "state": "ready" }));
    } else {
        println!("unclaimed  {}  → ready", resolved);
    }
    Ok(())
}

// ── integrate ─────────────────────────────────────────────────────────────────

fn cmd_integrate(partial: String, reason: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    // Warn if active children exist
    let active_children: Vec<&str> = graph
        .children(&resolved)
        .into_iter()
        .filter(|n| n.state != State::Integrated)
        .map(|n| n.id.as_str())
        .collect();
    if !active_children.is_empty() {
        eprintln!(
            "warning: {} has {} active child(ren): {}",
            resolved,
            active_children.len(),
            active_children.join(", ")
        );
    }

    // Before archiving: find latent nodes that `resolved` is blocking and that will
    // become fully unblocked once `resolved` integrates.
    let auto_surface: Vec<String> = graph
        .edges
        .iter()
        .filter(|e| e.from == resolved && e.edge_type == EdgeType::Blocks)
        .map(|e| e.to.clone())
        .filter(|y_id| {
            // Y must be latent
            graph
                .get_node(y_id)
                .is_some_and(|n| n.state == State::Latent)
            // Y must have no other non-integrated blockers besides `resolved`
            && !graph.edges.iter().any(|e| {
                e.to == *y_id
                    && e.edge_type == EdgeType::Blocks
                    && e.from != resolved
                    && graph
                        .get_node(&e.from)
                        .is_some_and(|b| b.state != State::Integrated)
            })
        })
        .collect();

    // Snapshot effective tags before archiving (parent tree may be gone later)
    let effective_tags = graph.effective_tags(&resolved);

    {
        let node = graph
            .get_node_mut(&resolved)
            .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;
        node.state = State::Integrated;
        // Denormalize: bake effective tags into the archived node
        node.tags = effective_tags;
        node.touch();
        if let Some(r) = &reason {
            set_reason(node, r);
        }
    }

    store::migrate_archive_if_needed(&root).ok();
    store::archive_node(&root, &mut graph, &resolved)?;

    // Promote newly unblocked latent nodes to ready
    for y_id in &auto_surface {
        if let Some(node) = graph.get_node_mut(y_id) {
            node.state = State::Ready;
            node.touch();
        }
    }

    store::save(&root, &graph)?;
    emit(&root, "integrate", &resolved, None, None, reason.as_deref());
    for y_id in &auto_surface {
        emit(
            &root,
            "surface",
            y_id,
            None,
            None,
            Some("auto-surfaced: last blocker integrated"),
        );
    }

    if json {
        let mut out = serde_json::json!({ "id": resolved, "state": "integrated" });
        if !auto_surface.is_empty() {
            out["newly_surfaced"] = serde_json::json!(auto_surface);
        }
        println!("{}", out);
    } else {
        println!("integrated  {}  → archive", resolved);
        for y_id in &auto_surface {
            println!("surfaced    {}  → ready (unblocked)", y_id);
        }
    }
    Ok(())
}

// ── shadow / unshadow ─────────────────────────────────────────────────────────

fn cmd_shadow(id: Option<String>, reason: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;

    match id {
        None => {
            let graph = store::load(&root)?;
            let shadowed: Vec<_> = graph.nodes.iter().filter(|n| n.shadowed).collect();

            if json {
                let out: Vec<_> = shadowed
                    .iter()
                    .map(|n| {
                        serde_json::json!({
                            "id": n.id, "title": n.title,
                            "state": n.state.to_string(), "part": n.part
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else if shadowed.is_empty() {
                println!("no shadowed nodes");
            } else {
                for n in shadowed {
                    println!("{:<20}  {:<40}  {}", n.id, n.title, n.state);
                }
            }
        }
        Some(partial) => {
            let mut graph = store::load(&root)?;
            let resolved = id::resolve(&graph, &partial)?;
            let node = graph
                .get_node_mut(&resolved)
                .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;
            node.shadowed = true;
            node.touch();
            if let Some(r) = &reason {
                set_reason(node, r);
            }
            store::save(&root, &graph)?;
            emit(&root, "shadow", &resolved, None, None, reason.as_deref());

            if json {
                println!("{}", serde_json::json!({ "id": resolved, "shadowed": true }));
            } else {
                println!("shadowed  {}", resolved);
            }
        }
    }
    Ok(())
}

fn cmd_unshadow(partial: String, reason: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    let node = graph
        .get_node_mut(&resolved)
        .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;
    node.shadowed = false;
    node.touch();
    if let Some(r) = &reason {
        set_reason(node, r);
    }
    store::save(&root, &graph)?;
    emit(&root, "unshadow", &resolved, None, None, reason.as_deref());

    if json {
        println!("{}", serde_json::json!({ "id": resolved, "shadowed": false }));
    } else {
        println!("unshadowed  {}", resolved);
    }
    Ok(())
}

// ── show ──────────────────────────────────────────────────────────────────────

fn cmd_show(partial: String, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    let node = graph
        .get_node(&resolved)
        .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;

    let blockers: Vec<&str> = graph
        .edges
        .iter()
        .filter(|e| e.to == resolved && e.edge_type == EdgeType::Blocks)
        .map(|e| e.from.as_str())
        .collect();

    let blocking: Vec<&str> = graph
        .edges
        .iter()
        .filter(|e| e.from == resolved && e.edge_type == EdgeType::Blocks)
        .map(|e| e.to.as_str())
        .collect();

    let children = graph.children(&resolved);

    let effective_tags = graph.effective_tags(&resolved);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "id": node.id,
                "title": node.title,
                "state": node.state.to_string(),
                "shadowed": node.shadowed,
                "part": node.part,
                "filed_by": node.filed_by,
                "tags": node.tags,
                "effective_tags": effective_tags,
                "meta": node.meta,
                "created_at": node.created_at,
                "updated_at": node.updated_at,
                "body": node.body,
                "blockers": blockers,
                "blocking": blocking,
                "children": children.iter().map(|n| &n.id).collect::<Vec<_>>(),
            })
        );
    } else {
        println!("id:       {}", node.id);
        println!("title:    {}", node.title);
        println!(
            "state:    {}{}",
            node.state,
            if node.shadowed { "  [shadowed]" } else { "" }
        );
        if let Some(p) = &node.part {
            println!("part:     {}", p);
        }
        if let Some(f) = &node.filed_by {
            println!("filed by: {}", f);
        }
        if !effective_tags.is_empty() {
            let own_set: std::collections::HashSet<&String> = node.tags.iter().collect();
            let display: Vec<String> = effective_tags.iter().map(|t| {
                if own_set.contains(t) { t.clone() } else { format!("{} (inherited)", t) }
            }).collect();
            println!("tags:     {}", display.join(", "));
        }
        if let Some(r) = node.meta.as_ref().and_then(|m| m["_reason"].as_str()) {
            println!("reason:   {}", r);
        }
        println!("created:  {}", node.created_at.format("%Y-%m-%d %H:%M"));
        println!("updated:  {}", node.updated_at.format("%Y-%m-%d %H:%M"));
        if !blockers.is_empty() {
            println!("blocked by: {}", blockers.join(", "));
        }
        if !blocking.is_empty() {
            println!("blocking:   {}", blocking.join(", "));
        }
        if !children.is_empty() {
            println!(
                "children: {}",
                children
                    .iter()
                    .map(|n| n.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if let Some(meta) = &node.meta {
            println!("meta:     {}", serde_json::to_string(meta)?);
        }
        if let Some(body) = &node.body {
            println!("\n{}", body);
        }
    }
    Ok(())
}

// ── tree ──────────────────────────────────────────────────────────────────────

fn cmd_tree(root_id: Option<String>, tag_filter: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;

    if json {
        fn node_to_tree(graph: &model::Graph, node: &model::Node, tag_filter: &Option<String>) -> Option<serde_json::Value> {
            if let Some(tag) = tag_filter
                && !subtree_has_tag(graph, &node.id, tag) {
                    return None;
            }
            let mut children = graph.children(&node.id);
            children.sort_by(|a, b| a.id.cmp(&b.id));
            let child_trees: Vec<serde_json::Value> = children
                .iter()
                .filter_map(|c| node_to_tree(graph, c, tag_filter))
                .collect();
            let effective = graph.effective_tags(&node.id);
            Some(serde_json::json!({
                "id": node.id,
                "title": node.title,
                "state": node.state.to_string(),
                "shadowed": node.shadowed,
                "part": node.part,
                "filed_by": node.filed_by,
                "tags": node.tags,
                "effective_tags": effective,
                "children": child_trees,
            }))
        }

        let tree_roots: Vec<&model::Node> = match &root_id {
            Some(partial) => {
                let resolved = id::resolve(&graph, partial)?;
                vec![graph
                    .get_node(&resolved)
                    .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?]
            }
            None => graph.roots(),
        };

        let out: Vec<serde_json::Value> = tree_roots
            .iter()
            .filter_map(|r| node_to_tree(&graph, r, &tag_filter))
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    let roots: Vec<&model::Node> = match &root_id {
        Some(partial) => {
            let resolved = id::resolve(&graph, partial)?;
            vec![graph
                .get_node(&resolved)
                .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?]
        }
        None => graph.roots(),
    };

    fn print_node(graph: &model::Graph, node: &model::Node, depth: usize, tag_filter: &Option<String>) {
        if let Some(tag) = tag_filter
            && !subtree_has_tag(graph, &node.id, tag) {
                return;
        }
        let indent = "  ".repeat(depth);
        let shadow = if node.shadowed { " [shadowed]" } else { "" };
        let part = node
            .part
            .as_deref()
            .map(|p| format!("  :{}", p))
            .unwrap_or_default();
        let tags_str = if node.tags.is_empty() {
            String::new()
        } else {
            format!("  #{}", node.tags.join(" #"))
        };
        let leaf = if depth > 0 {
            node.id.rfind('.').map(|i| &node.id[i + 1..]).unwrap_or(&node.id)
        } else {
            &node.id
        };
        println!(
            "{}{}  {}  [{}{}]{}{}",
            indent, leaf, node.title, node.state, shadow, part, tags_str
        );
        let mut children = graph.children(&node.id);
        children.sort_by(|a, b| a.id.cmp(&b.id));
        for child in children {
            print_node(graph, child, depth + 1, tag_filter);
        }
    }

    for node in roots {
        print_node(&graph, node, 0, &tag_filter);
    }
    Ok(())
}

// ── parts ─────────────────────────────────────────────────────────────────────

fn cmd_parts(json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;
    let conn = db::materialize(&graph)?;
    let parts = db::parts_summary(&conn)?;

    if json {
        let out: Vec<_> = parts
            .iter()
            .map(|p| serde_json::json!({ "part": p.part, "count": p.count, "ids": p.ids }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if parts.is_empty() {
        println!("no claimed nodes");
    } else {
        for p in &parts {
            println!("{:<30}  {} node(s):  {}", p.part, p.count, p.ids);
        }
    }
    Ok(())
}

// ── therapy ───────────────────────────────────────────────────────────────────

fn cmd_therapy(json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;
    let conn = db::materialize(&graph)?;
    let nodes = db::therapy_nodes(&conn)?;

    // Detect orphan body files (*.md in issues/ with no matching node)
    let orphans = store::find_orphan_bodies(&root, &graph)?;

    if json {
        let mut out: Vec<_> = nodes
            .iter()
            .map(|n| {
                let user_reason = graph.get_node(&n.id)
                    .and_then(|node| node.meta.as_ref())
                    .and_then(|m| m["_reason"].as_str());
                let mut obj = serde_json::json!({
                    "id": n.id, "title": n.title,
                    "part": n.part, "updated_at": n.updated_at,
                    "reason": n.reason
                });
                if let Some(r) = user_reason {
                    obj["_reason"] = serde_json::json!(r);
                }
                obj
            })
            .collect();
        for path in &orphans {
            out.push(serde_json::json!({
                "id": path, "title": "(orphan body file)",
                "reason": "orphan"
            }));
        }
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if nodes.is_empty() && orphans.is_empty() {
        println!("all clear");
    } else {
        for n in &nodes {
            let part = n.part.as_deref().unwrap_or("—");
            let user_reason = graph.get_node(&n.id)
                .and_then(|node| node.meta.as_ref())
                .and_then(|m| m["_reason"].as_str());
            match user_reason {
                Some(r) => println!(
                    "{:<20}  {:<40}  {:<20}  {} — {}",
                    n.id, n.title, part, n.reason, r
                ),
                None => println!(
                    "{:<20}  {:<40}  {:<20}  {}",
                    n.id, n.title, part, n.reason
                ),
            }
        }
        for path in &orphans {
            println!("{:<20}  {:<40}  {:<20}  orphan", path, "(body file)", "—");
        }
    }
    Ok(())
}

// ── rm ────────────────────────────────────────────────────────────────────

// ── move ──────────────────────────────────────────────────────────────────────

fn cmd_move(partial: String, new_parent: Option<String>, to_root: bool, reason: Option<String>, json: bool) -> Result<()> {
    if new_parent.is_none() && !to_root {
        bail!("provide a new parent ID, or use --root to promote to root level");
    }
    if new_parent.is_some() && to_root {
        bail!("cannot specify both a new parent and --root");
    }

    let root = store::find_root()?;
    let mut graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    // Resolve new parent if given
    let new_parent_id = match &new_parent {
        Some(p) => {
            let pid = id::resolve(&graph, p)?;
            // Can't move under yourself or your own descendant
            if pid == resolved || pid.starts_with(&format!("{}.", resolved)) {
                bail!("cannot move {} under its own descendant {}", resolved, pid);
            }
            Some(pid)
        }
        None => None,
    };

    // Extract the short (leaf) segment of the node's current ID
    let short = resolved.rsplit('.').next().unwrap();
    let new_id = match &new_parent_id {
        Some(pid) => format!("{}.{}", pid, short),
        None => short.to_string(),
    };

    if new_id == resolved {
        bail!("{} is already there", resolved);
    }

    // Check for ID collision
    if graph.get_node(&new_id).is_some() {
        bail!("a node with id {} already exists", new_id);
    }

    // Collect all nodes to rename: the node itself + all descendants
    let old_prefix = format!("{}.", resolved);
    let renames: Vec<(String, String)> = graph
        .nodes
        .iter()
        .filter(|n| n.id == resolved || n.id.starts_with(&old_prefix))
        .map(|n| {
            let new = if n.id == resolved {
                new_id.clone()
            } else {
                format!("{}{}", new_id, &n.id[resolved.len()..])
            };
            (n.id.clone(), new)
        })
        .collect();

    // Check for collisions with descendants too
    for (_, new) in &renames {
        if graph.nodes.iter().any(|n| n.id == *new && !renames.iter().any(|(old, _)| *old == n.id)) {
            bail!("a node with id {} already exists", new);
        }
    }

    // Rename body files
    let issues_dir = root.join("issues");
    for (old, new) in &renames {
        let src = issues_dir.join(format!("{}.md", old));
        let dst = issues_dir.join(format!("{}.md", new));
        if src.exists() {
            std::fs::rename(&src, &dst)?;
        }
    }

    // Update node IDs
    for node in &mut graph.nodes {
        if let Some((_, new)) = renames.iter().find(|(old, _)| *old == node.id) {
            node.id = new.clone();
            node.touch();
        }
    }

    // Update edge references
    for edge in &mut graph.edges {
        if let Some((_, new)) = renames.iter().find(|(old, _)| *old == edge.from) {
            edge.from = new.clone();
        }
        if let Some((_, new)) = renames.iter().find(|(old, _)| *old == edge.to) {
            edge.to = new.clone();
        }
    }

    store::save(&root, &graph)?;
    emit(&root, "move", &new_id, None, Some(&resolved), reason.as_deref());

    if json {
        let moved: Vec<_> = renames.iter().map(|(old, new)| serde_json::json!({"old": old, "new": new})).collect();
        println!("{}", serde_json::json!({ "id": new_id, "from": resolved, "moved": moved }));
    } else {
        for (old, new) in &renames {
            println!("{}  →  {}", old, new);
        }
    }
    Ok(())
}

fn cmd_rm(partial: String, reason: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    // Refuse to remove nodes with active children
    let active_children: Vec<&str> = graph
        .children(&resolved)
        .into_iter()
        .filter(|n| n.state != State::Integrated)
        .map(|n| n.id.as_str())
        .collect();
    if !active_children.is_empty() {
        bail!(
            "{} has {} active child(ren): {} — remove them first",
            resolved,
            active_children.len(),
            active_children.join(", ")
        );
    }

    let title = graph
        .get_node(&resolved)
        .map(|n| n.title.clone())
        .unwrap_or_default();

    // Archive the node (moves to archive.jsonl + body to archive/)
    store::migrate_archive_if_needed(&root).ok();
    store::archive_node(&root, &mut graph, &resolved)?;
    store::save(&root, &graph)?;

    emit(&root, "rm", &resolved, None, Some(&title), reason.as_deref());

    if json {
        println!("{}", serde_json::json!({ "id": resolved, "removed": true }));
    } else {
        println!("removed  {}  {}", resolved, title);
    }
    Ok(())
}

// ── rename ────────────────────────────────────────────────────────────────────

fn cmd_rename(partial: String, title: String, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    let node = graph
        .get_node_mut(&resolved)
        .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;

    node.title = title.clone();
    node.touch();
    store::save(&root, &graph)?;
    emit(&root, "rename", &resolved, None, Some(&title), None);

    if json {
        println!("{}", serde_json::json!({ "id": resolved, "title": title }));
    } else {
        println!("renamed  {}  → {}", resolved, title);
    }
    Ok(())
}

// ── edit ──────────────────────────────────────────────────────────────────────

fn cmd_edit(partial: String, body: Option<String>, file: Option<String>, force_editor: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    let existing = store::read_body(&root, &resolved)?.unwrap_or_default();

    // Determine body content from flags or auto-detect
    let updated = if let Some(text) = body {
        text
    } else if let Some(path) = file {
        std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("cannot read {}: {}", path, e))?
    } else if force_editor || std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        // Interactive: open $EDITOR
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let tmp = std::env::temp_dir().join(format!("cx-{}.md", resolved));
        std::fs::write(&tmp, &existing)?;
        std::process::Command::new(&editor).arg(&tmp).status()?;
        std::fs::read_to_string(&tmp)?
    } else {
        // Non-interactive: read from stdin
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    };

    if updated != existing {
        store::write_body(&root, &resolved, &updated)?;
        emit(&root, "edit", &resolved, None, None, None);
        println!("saved  {}", resolved);
    } else {
        println!("no changes");
    }
    Ok(())
}

// ── edges ─────────────────────────────────────────────────────────────────────

fn cmd_edge(
    a_partial: String,
    b_partial: String,
    edge_type: EdgeType,
    remove: bool,
    json: bool,
) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;

    let a = id::resolve(&graph, &a_partial)?;
    let b = id::resolve(&graph, &b_partial)?;

    // Validate both endpoints exist
    if graph.get_node(&a).is_none() {
        bail!("node not found: {}", a);
    }
    if graph.get_node(&b).is_none() {
        bail!("node not found: {}", b);
    }

    // Cycle check for blocks edges
    if !remove && edge_type == EdgeType::Blocks && graph.would_cycle(&a, &b) {
        bail!("adding {} --blocks→ {} would create a cycle", a, b);
    }

    if remove {
        graph
            .edges
            .retain(|e| !(e.from == a && e.to == b && e.edge_type == edge_type));
        store::save(&root, &graph)?;
        if json {
            println!(
                "{}",
                serde_json::json!({ "removed": { "from": a, "to": b, "type": edge_type.to_string() } })
            );
        } else {
            println!("removed  {} --{}→ {}", a, edge_type, b);
        }
    } else {
        let exists = graph
            .edges
            .iter()
            .any(|e| e.from == a && e.to == b && e.edge_type == edge_type);
        if !exists {
            graph.edges.push(model::Edge {
                from: a.clone(),
                to: b.clone(),
                edge_type: edge_type.clone(),
            });
            store::save(&root, &graph)?;
        }
        if json {
            println!(
                "{}",
                serde_json::json!({ "added": { "from": a, "to": b, "type": edge_type.to_string() } })
            );
        } else {
            println!("added  {} --{}→ {}", a, edge_type, b);
        }
    }
    Ok(())
}

// ── tag / untag / tags ────────────────────────────────────────────────────

fn cmd_tag(partial: String, tag: String, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    let node = graph
        .get_node_mut(&resolved)
        .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;
    if !node.tags.contains(&tag) {
        node.tags.push(tag.clone());
        node.tags.sort();
        node.touch();
    }
    store::save(&root, &graph)?;
    emit(&root, "tag", &resolved, None, Some(&tag), None);

    if json {
        println!("{}", serde_json::json!({ "id": resolved, "tag": tag }));
    } else {
        println!("tagged  {}  +{}", resolved, tag);
    }
    Ok(())
}

fn cmd_untag(partial: String, tag: String, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    let node = graph
        .get_node_mut(&resolved)
        .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;
    let before = node.tags.len();
    node.tags.retain(|t| t != &tag);
    if node.tags.len() < before {
        node.touch();
    }
    store::save(&root, &graph)?;
    emit(&root, "untag", &resolved, None, Some(&tag), None);

    if json {
        println!("{}", serde_json::json!({ "id": resolved, "tag": tag }));
    } else {
        println!("untagged  {}  -{}", resolved, tag);
    }
    Ok(())
}

fn cmd_tags(partial: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;

    match partial {
        Some(p) => {
            let resolved = id::resolve(&graph, &p)?;
            let own = graph
                .get_node(&resolved)
                .map(|n| n.tags.clone())
                .unwrap_or_default();
            let effective = graph.effective_tags(&resolved);
            if json {
                println!("{}", serde_json::json!({
                    "id": resolved,
                    "own": own,
                    "effective": effective,
                }));
            } else if effective.is_empty() {
                println!("no tags on {}", resolved);
            } else {
                for t in &effective {
                    let marker = if own.contains(t) { "" } else { " (inherited)" };
                    println!("  {}{}", t, marker);
                }
            }
        }
        None => {
            // List all tags in use across the graph
            let mut all_tags = std::collections::BTreeSet::new();
            for node in &graph.nodes {
                all_tags.extend(node.tags.iter().cloned());
            }
            if json {
                let tags: Vec<&String> = all_tags.iter().collect();
                println!("{}", serde_json::to_string_pretty(&tags)?);
            } else if all_tags.is_empty() {
                println!("no tags in use");
            } else {
                for t in &all_tags {
                    println!("  {}", t);
                }
            }
        }
    }
    Ok(())
}

// ── find ──────────────────────────────────────────────────────────────────

fn cmd_find(query: String, tag_filter: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;
    let q = query.to_lowercase();

    let matches: Vec<&model::Node> = graph
        .nodes
        .iter()
        .filter(|n| n.title.to_lowercase().contains(&q))
        .filter(|n| match &tag_filter {
            Some(t) => graph.effective_tags(&n.id).contains(t),
            None => true,
        })
        .collect();

    if json {
        let out: Vec<_> = matches
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id, "title": n.title,
                    "state": n.state.to_string(), "part": n.part,
                    "filed_by": n.filed_by,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if matches.is_empty() {
        println!("no nodes matching '{}'", query);
    } else {
        for n in &matches {
            let part = n.part.as_deref().unwrap_or("—");
            println!("{:<20}  {:<40}  {:<12}  {}", n.id, n.title, n.state, part);
        }
    }
    Ok(())
}

// ── list ──────────────────────────────────────────────────────────────────────

fn cmd_list(state_filter: Option<String>, filed_by_filter: Option<String>, tag_filter: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;

    let nodes: Vec<&model::Node> = graph
        .nodes
        .iter()
        .filter(|n| match &state_filter {
            Some(s) => n.state.to_string() == *s,
            None => true,
        })
        .filter(|n| match &filed_by_filter {
            Some(f) => n.filed_by.as_deref() == Some(f.as_str()),
            None => true,
        })
        .filter(|n| match &tag_filter {
            Some(t) => graph.effective_tags(&n.id).contains(t),
            None => true,
        })
        .collect();

    if json {
        let out: Vec<_> = nodes
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id, "title": n.title,
                    "state": n.state.to_string(),
                    "shadowed": n.shadowed, "part": n.part,
                    "filed_by": n.filed_by,
                    "tags": n.tags,
                    "effective_tags": graph.effective_tags(&n.id),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if nodes.is_empty() {
        println!("no nodes{}", state_filter.map(|s| format!(" with state={}", s)).unwrap_or_default());
    } else {
        for n in &nodes {
            let shadow = if n.shadowed { " [shadowed]" } else { "" };
            let part = n.part.as_deref().unwrap_or("—");
            let tags_str = if n.tags.is_empty() {
                String::new()
            } else {
                format!("  #{}", n.tags.join(" #"))
            };
            println!("{:<20}  {:<40}  {:<12}  {}{}{}", n.id, n.title, n.state, part, shadow, tags_str);
        }
    }
    Ok(())
}

// ── log ───────────────────────────────────────────────────────────────────────

fn cmd_log(limit: usize, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let events = store::recent_events(&root, limit)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&events)?);
    } else if events.is_empty() {
        println!("no events");
    } else {
        for e in &events {
            let ts = e["ts"].as_str().unwrap_or("?");
            let action = e["action"].as_str().unwrap_or("?");
            let node_id = e["node_id"].as_str().unwrap_or("?");
            let part = e["part"].as_str().unwrap_or("");
            let detail = e["detail"].as_str().unwrap_or("");
            let reason = e["reason"].as_str().unwrap_or("");
            let mut extra = match (part, detail) {
                ("", "") => String::new(),
                (p, "") => format!("  {}", p),
                ("", d) => format!("  {}", d),
                (p, d) => format!("  {}  {}", p, d),
            };
            if !reason.is_empty() {
                extra.push_str(&format!("  ({})", reason));
            }
            // Show only date+time, not full RFC3339
            let ts_short = &ts[..19].replace('T', " ");
            println!("{}  {:<12}  {}{}", ts_short, action, node_id, extra);
        }
    }
    Ok(())
}

// ── agents ────────────────────────────────────────────────────────────────────

fn cmd_agents(json: bool) -> Result<()> {
    let root = store::find_root()?;
    let agents = store::load_agents(&root)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agents)?);
    } else if agents.is_empty() {
        println!("no agents registered");
    } else {
        for a in &agents {
            let ts_short = &a.last_seen[..19].replace('T', " ");
            println!("{:<30}  last seen {}", a.name, ts_short);
        }
    }
    Ok(())
}

// ── status ────────────────────────────────────────────────────────────────────

fn cmd_status(json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;
    let conn = db::materialize(&graph)?;
    let ready = db::ready_nodes(&conn)?;

    if json {
        fn node_to_tree(graph: &model::Graph, node: &model::Node) -> serde_json::Value {
            let mut children = graph.children(&node.id);
            children.sort_by(|a, b| a.id.cmp(&b.id));
            let child_trees: Vec<serde_json::Value> = children
                .iter()
                .map(|c| node_to_tree(graph, c))
                .collect();
            let effective = graph.effective_tags(&node.id);
            serde_json::json!({
                "id": node.id,
                "title": node.title,
                "state": node.state.to_string(),
                "shadowed": node.shadowed,
                "part": node.part,
                "filed_by": node.filed_by,
                "tags": node.tags,
                "effective_tags": effective,
                "children": child_trees,
            })
        }

        let tree: Vec<serde_json::Value> = graph
            .roots()
            .iter()
            .map(|r| node_to_tree(&graph, r))
            .collect();
        let ready_out: Vec<_> = ready
            .iter()
            .map(|n| serde_json::json!({ "id": n.id, "title": n.title, "part": n.part }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "tree": tree,
                "ready": ready_out,
            }))?
        );
    } else {
        fn print_node(graph: &model::Graph, node: &model::Node, depth: usize) {
            let indent = "  ".repeat(depth);
            let shadow = if node.shadowed { " [shadowed]" } else { "" };
            let part = node
                .part
                .as_deref()
                .map(|p| format!("  :{}", p))
                .unwrap_or_default();
            let tags_str = if node.tags.is_empty() {
                String::new()
            } else {
                format!("  #{}", node.tags.join(" #"))
            };
            let leaf = if depth > 0 {
                node.id.rfind('.').map(|i| &node.id[i + 1..]).unwrap_or(&node.id)
            } else {
                &node.id
            };
            println!(
                "{}{}  {}  [{}{}]{}{}",
                indent, leaf, node.title, node.state, shadow, part, tags_str
            );
            let mut children = graph.children(&node.id);
            children.sort_by(|a, b| a.id.cmp(&b.id));
            for child in children {
                print_node(graph, child, depth + 1);
            }
        }

        let roots = graph.roots();
        if roots.is_empty() {
            println!("(no nodes)");
        } else {
            for node in roots {
                print_node(&graph, node, 0);
            }
        }

        println!();

        if ready.is_empty() {
            println!("no ready nodes");
        } else {
            for n in &ready {
                let part = n.part.as_deref().unwrap_or("—");
                println!("{:<20}  {:<40}  {}", n.id, n.title, part);
            }
        }
    }
    Ok(())
}

// ── meta ──────────────────────────────────────────────────────────────────────

fn cmd_meta(partial: String, value: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;

    match value {
        None => {
            let graph = store::load(&root)?;
            let resolved = id::resolve(&graph, &partial)?;
            let node = graph
                .get_node(&resolved)
                .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;
            let meta = node.meta.as_ref().unwrap_or(&serde_json::Value::Null);
            println!("{}", serde_json::to_string_pretty(meta)?);
        }
        Some(raw) => {
            let parsed: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|e| anyhow::anyhow!("invalid JSON: {}", e))?;
            let mut graph = store::load(&root)?;
            let resolved = id::resolve(&graph, &partial)?;
            let node = graph
                .get_node_mut(&resolved)
                .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;
            node.meta = Some(parsed.clone());
            node.touch();
            store::save(&root, &graph)?;
            if json {
                println!("{}", serde_json::json!({ "id": resolved, "meta": parsed }));
            } else {
                println!("meta set  {}", resolved);
            }
        }
    }
    Ok(())
}
