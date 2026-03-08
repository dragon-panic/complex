use anyhow::Result;
use rusqlite::{params, Connection};

use crate::model::Graph;

pub fn materialize(graph: &Graph) -> Result<Connection> {
    let conn = Connection::open_in_memory()?;

    conn.execute_batch(
        "CREATE TABLE nodes (
            id         TEXT PRIMARY KEY,
            title      TEXT NOT NULL,
            state      TEXT NOT NULL,
            shadowed   INTEGER NOT NULL DEFAULT 0,
            part       TEXT,
            filed_by   TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE edges (
            from_id TEXT NOT NULL,
            to_id   TEXT NOT NULL,
            type    TEXT NOT NULL,
            PRIMARY KEY (from_id, to_id, type)
        );",
    )?;

    for n in &graph.nodes {
        conn.execute(
            "INSERT INTO nodes VALUES (?,?,?,?,?,?,?,?)",
            params![
                n.id,
                n.title,
                n.state.to_string(),
                n.shadowed as i32,
                n.part,
                n.filed_by,
                n.created_at.to_rfc3339(),
                n.updated_at.to_rfc3339(),
            ],
        )?;
    }

    for e in &graph.edges {
        conn.execute(
            "INSERT INTO edges VALUES (?,?,?)",
            params![e.from, e.to, e.edge_type.to_string()],
        )?;
    }

    Ok(conn)
}

pub struct ReadyNode {
    pub id: String,
    pub title: String,
    pub part: Option<String>,
}

/// Nodes that are ready and have no unresolved blocking dependencies.
pub fn ready_nodes(conn: &Connection) -> Result<Vec<ReadyNode>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, part FROM nodes
         WHERE state = 'ready'
           AND shadowed = 0
           AND NOT EXISTS (
               SELECT 1 FROM edges e
               JOIN nodes b ON e.from_id = b.id
               JOIN nodes ancestor ON e.to_id = ancestor.id
               WHERE (nodes.id = ancestor.id OR nodes.id LIKE ancestor.id || '.%')
                 AND e.type = 'blocks'
                 AND b.state != 'integrated'
           )
         ORDER BY updated_at ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ReadyNode {
            id: row.get(0)?,
            title: row.get(1)?,
            part: row.get(2)?,
        })
    })?;

    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub struct TherapyNode {
    pub id: String,
    pub title: String,
    pub part: Option<String>,
    pub updated_at: String,
    pub reason: &'static str,
}

/// Stale claimed nodes and shadowed nodes needing attention.
pub fn therapy_nodes(conn: &Connection) -> Result<Vec<TherapyNode>> {
    let mut results = vec![];

    // Stale: claimed but not touched in 24h
    let mut stmt = conn.prepare(
        "SELECT id, title, part, updated_at FROM nodes
         WHERE state = 'claimed'
           AND datetime(updated_at) < datetime('now', '-24 hours')
         ORDER BY updated_at ASC",
    )?;
    let stale = stmt.query_map([], |row| {
        Ok(TherapyNode {
            id: row.get(0)?,
            title: row.get(1)?,
            part: row.get::<_, Option<String>>(2)?,
            updated_at: row.get(3)?,
            reason: "stale (claimed > 24h)",
        })
    })?;
    for n in stale {
        results.push(n?);
    }

    // Shadowed nodes
    let mut stmt = conn.prepare(
        "SELECT id, title, part, updated_at FROM nodes
         WHERE shadowed = 1
         ORDER BY updated_at ASC",
    )?;
    let shadowed = stmt.query_map([], |row| {
        Ok(TherapyNode {
            id: row.get(0)?,
            title: row.get(1)?,
            part: row.get::<_, Option<String>>(2)?,
            updated_at: row.get(3)?,
            reason: "shadowed",
        })
    })?;
    for n in shadowed {
        results.push(n?);
    }

    Ok(results)
}

pub struct PartEntry {
    pub part: String,
    pub ids: String,
    pub count: i64,
}

pub fn parts_summary(conn: &Connection) -> Result<Vec<PartEntry>> {
    let mut stmt = conn.prepare(
        "SELECT part, GROUP_CONCAT(id, ', '), COUNT(*) FROM nodes
         WHERE state = 'claimed' AND part IS NOT NULL
         GROUP BY part
         ORDER BY part",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(PartEntry {
            part: row.get(0)?,
            ids: row.get(1)?,
            count: row.get(2)?,
        })
    })?;

    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
