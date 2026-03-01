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
    Init,

    /// Create a new root complex (human-facing entry point)
    Add { title: Vec<String> },

    /// Create a child node under a parent
    New {
        parent_id: String,
        title: Vec<String>,
    },

    /// List ready nodes, or promote a latent node to ready
    Surface {
        id: Option<String>,
        /// Why this node is being surfaced
        #[arg(long)]
        reason: Option<String>,
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
    Tree { id: Option<String> },

    /// Show claimed nodes grouped by part
    Parts,

    /// Show stale or stuck nodes needing review
    Therapy,

    /// Open a node's body in $EDITOR
    Edit { id: String },

    /// Declare that node A blocks node B
    Block { a: String, b: String },

    /// Remove a blocks edge
    Unblock { a: String, b: String },

    /// Add a related (non-blocking) edge between two nodes
    Relate { a: String, b: String },

    /// Mark node A as discovered while working on node B
    Discover { a: String, b: String },

    /// List all nodes, optionally filtered by state
    List {
        #[arg(long, value_name = "STATE")]
        state: Option<String>,
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
        Commands::Init => cmd_init(),
        Commands::Add { title } => cmd_add(title.join(" "), cli.json),
        Commands::New { parent_id, title } => cmd_new(parent_id, title.join(" "), cli.json),
        Commands::Surface { id, reason } => cmd_surface(id, reason, cli.json),
        Commands::Claim { id, r#as, reason } => cmd_claim(id, r#as, reason, cli.json),
        Commands::Unclaim { id, reason } => cmd_unclaim(id, reason, cli.json),
        Commands::Integrate { id, reason } => cmd_integrate(id, reason, cli.json),
        Commands::Shadow { id, reason } => cmd_shadow(id, reason, cli.json),
        Commands::Unshadow { id, reason } => cmd_unshadow(id, reason, cli.json),
        Commands::Show { id } => cmd_show(id, cli.json),
        Commands::Tree { id } => cmd_tree(id, cli.json),
        Commands::Parts => cmd_parts(cli.json),
        Commands::Therapy => cmd_therapy(cli.json),
        Commands::Edit { id } => cmd_edit(id),
        Commands::Block { a, b } => cmd_edge(a, b, EdgeType::Blocks, false, cli.json),
        Commands::Unblock { a, b } => cmd_edge(a, b, EdgeType::Blocks, true, cli.json),
        Commands::Relate { a, b } => cmd_edge(a, b, EdgeType::Related, false, cli.json),
        Commands::Discover { a, b } => cmd_edge(a, b, EdgeType::DiscoveredFrom, false, cli.json),
        Commands::List { state } => cmd_list(state, cli.json),
        Commands::Agent => { print!("{}", AGENT_GUIDE); Ok(()) },
        Commands::Meta { id, value } => cmd_meta(id, value, cli.json),
        Commands::Log { limit } => cmd_log(limit, cli.json),
        Commands::Agents => cmd_agents(cli.json),
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
cx surface --json                 ready tasks (no open blockers)
cx claim <id> --as <name>         take ownership (or set CX_PART env var)
cx unclaim <id>                   release if you cannot complete it
cx integrate <id>                 mark done → archive, unblocks dependents
cx new <parent-id> <title>        create a child task under a parent
cx discover <new-id> <source-id>  record task found while working on source
cx shadow <id>                    flag as blocked/stuck
cx show <id> --json               full node detail: state, edges, body, children
cx tree --json                    full hierarchy with states
cx parts --json                   what each part currently holds
cx therapy --json                 stale (claimed >24h) and shadowed nodes
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

## IDs

Hierarchy is encoded in the ID:
  a3F2              root complex
  a3F2.bX7c         child task
  a3F2.bX7c.Qd4e   grandchild subtask

Short IDs (leaf segment) work when unambiguous:  cx claim bX7c

## Environment

  CX_PART   your identity — set this before claiming anything

## What to commit

After any cx mutation, stage and commit `.complex/`:
  git add .complex/ && git commit -m "claim(bX7c): implement JWT tokens"
  git add .complex/ && git commit -m "integrate(bX7c): implement JWT tokens"
"#;

// ── init ─────────────────────────────────────────────────────────────────────

fn cmd_init() -> Result<()> {
    let cwd = std::env::current_dir()?;
    store::init(&cwd)?;
    println!("initialized .complex/ in {}", cwd.display());
    Ok(())
}

// ── add / new ─────────────────────────────────────────────────────────────────

fn cmd_add(title: String, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;

    let new_id = id::generate(None);
    let node = model::Node::new(new_id.clone(), title.clone());
    graph.nodes.push(node);
    store::save(&root, &graph)?;
    emit(&root, "create", &new_id, None, Some(&title), None);

    if json {
        println!("{}", serde_json::json!({ "id": new_id, "title": title }));
    } else {
        println!("created  {}  {}", new_id, title);
    }
    Ok(())
}

fn cmd_new(parent_partial: String, title: String, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let mut graph = store::load(&root)?;

    let parent_id = id::resolve(&graph, &parent_partial)
        .map_err(|_| anyhow::anyhow!("parent '{}' not found — use cx tree to list nodes", parent_partial))?;
    let new_id = id::generate(Some(&parent_id));
    let node = model::Node::new(new_id.clone(), title.clone());
    graph.nodes.push(node);
    store::save(&root, &graph)?;
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

fn cmd_surface(id: Option<String>, reason: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;

    match id {
        None => {
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
        }
        Some(partial) => {
            let mut graph = store::load(&root)?;
            let resolved = id::resolve(&graph, &partial)?;
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
            store::save(&root, &graph)?;
            emit(&root, "surface", &resolved, None, None, reason.as_deref());

            if json {
                println!("{}", serde_json::json!({ "id": resolved, "state": "ready" }));
            } else {
                println!("surfaced  {}  → ready", resolved);
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

    let node = graph
        .get_node_mut(&resolved)
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

    {
        let node = graph
            .get_node_mut(&resolved)
            .ok_or_else(|| anyhow::anyhow!("node not found: {}", resolved))?;
        node.state = State::Integrated;
        node.touch();
        if let Some(r) = &reason {
            set_reason(node, r);
        }
    }

    store::migrate_archive_if_needed(&root).ok();
    store::archive_node(&root, &mut graph, &resolved)?;
    store::save(&root, &graph)?;
    emit(&root, "integrate", &resolved, None, None, reason.as_deref());

    if json {
        println!("{}", serde_json::json!({ "id": resolved, "state": "integrated" }));
    } else {
        println!("integrated  {}  → archive", resolved);
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

    if json {
        println!(
            "{}",
            serde_json::json!({
                "id": node.id,
                "title": node.title,
                "state": node.state.to_string(),
                "shadowed": node.shadowed,
                "part": node.part,
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

fn cmd_tree(root_id: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&graph.nodes)?);
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

    fn print_node(graph: &model::Graph, node: &model::Node, depth: usize) {
        let indent = "  ".repeat(depth);
        let shadow = if node.shadowed { " [shadowed]" } else { "" };
        let part = node
            .part
            .as_deref()
            .map(|p| format!("  :{}", p))
            .unwrap_or_default();
        let leaf = if depth > 0 {
            node.id.rfind('.').map(|i| &node.id[i + 1..]).unwrap_or(&node.id)
        } else {
            &node.id
        };
        println!(
            "{}{}  {}  [{}{}]{}",
            indent, leaf, node.title, node.state, shadow, part
        );
        let mut children = graph.children(&node.id);
        children.sort_by(|a, b| a.id.cmp(&b.id));
        for child in children {
            print_node(graph, child, depth + 1);
        }
    }

    for node in roots {
        print_node(&graph, node, 0);
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

    if json {
        let out: Vec<_> = nodes
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
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if nodes.is_empty() {
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
    }
    Ok(())
}

// ── edit ──────────────────────────────────────────────────────────────────────

fn cmd_edit(partial: String) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;
    let resolved = id::resolve(&graph, &partial)?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let existing = store::read_body(&root, &resolved)?.unwrap_or_default();

    let tmp = std::env::temp_dir().join(format!("cx-{}.md", resolved));
    std::fs::write(&tmp, &existing)?;

    std::process::Command::new(&editor).arg(&tmp).status()?;

    let updated = std::fs::read_to_string(&tmp)?;
    if updated != existing {
        store::write_body(&root, &resolved, &updated)?;
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

// ── list ──────────────────────────────────────────────────────────────────────

fn cmd_list(state_filter: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let graph = store::load(&root)?;

    let nodes: Vec<&model::Node> = graph
        .nodes
        .iter()
        .filter(|n| match &state_filter {
            Some(s) => n.state.to_string() == *s,
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
            println!("{:<20}  {:<40}  {:<12}  {}{}", n.id, n.title, n.state, part, shadow);
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
