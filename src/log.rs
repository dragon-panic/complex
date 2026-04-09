use anyhow::{bail, Context, Result};

use crate::store;

pub fn cmd_log(limit: usize, since: Option<String>, json: bool) -> Result<()> {
    let root = store::find_root()?;
    let nodes_path = root.join("nodes");
    let issues_path = root.join("issues");

    // Build git log command
    let mut args = vec![
        "log".to_string(),
        "--pretty=format:%H%x00%aI%x00%s".to_string(),
    ];
    if let Some(ref sha) = since {
        args.push(format!("{}..HEAD", sha));
    }
    args.push(format!("-{}", limit));
    args.push("--".to_string());
    args.push(nodes_path.to_string_lossy().to_string());
    args.push(issues_path.to_string_lossy().to_string());

    let output = std::process::Command::new("git")
        .args(&args)
        .output()
        .context("failed to run git log — is this a git repository?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git log failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<(&str, &str, &str)> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\0');
            Some((parts.next()?, parts.next()?, parts.next()?))
        })
        .collect();

    if json {
        let out: Vec<serde_json::Value> = commits
            .iter()
            .map(|(hash, date, subject)| {
                let changes = diff_commit(hash, &root);
                serde_json::json!({
                    "hash": hash,
                    "date": date,
                    "subject": subject,
                    "changes": changes,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if commits.is_empty() {
        println!("no commits touching .complex/");
    } else {
        for (hash, date, subject) in &commits {
            let short = &hash[..7.min(hash.len())];
            let ts = &date[..19.min(date.len())].replace('T', " ");
            println!("{}  {}  {}", ts, short, subject);
            let changes = diff_commit(hash, &root);
            for c in &changes {
                let action = c["action"].as_str().unwrap_or("?");
                let node_id = c["node_id"].as_str().unwrap_or("?");
                match action {
                    "created" => {
                        let title = c["title"].as_str().unwrap_or("");
                        let state = c["state"].as_str().unwrap_or("latent");
                        let tags_str = c["tags"].as_array()
                            .filter(|t| !t.is_empty())
                            .map(|t| format!("  #{}", t.iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(" #")))
                            .unwrap_or_default();
                        println!("  + {}  {}  [{}]{}", node_id, title, state, tags_str);
                    }
                    "removed" => {
                        println!("  - {}", node_id);
                    }
                    "modified" => {
                        let fields = c["fields"].as_object();
                        let mut parts = vec![node_id.to_string()];
                        if let Some(fields) = fields {
                            for (key, val) in fields {
                                let from = val["from"].as_str().unwrap_or("?");
                                let to = val["to"].as_str().unwrap_or("?");
                                parts.push(format!("{}: {} → {}", key, from, to));
                            }
                        }
                        println!("  ~ {}", parts.join("  "));
                    }
                    "archived" => println!("  x {}  archived", node_id),
                    "unarchived" => println!("  o {}  unarchived", node_id),
                    "body_added" | "body_edited" => {
                        let summary = c["body"].as_str().map(first_line).unwrap_or_default();
                        if summary.is_empty() {
                            println!("  ~ {}  body", node_id);
                        } else {
                            println!("  ~ {}  body: {}", node_id, summary);
                        }
                    }
                    "body_removed" => println!("  - {}  body removed", node_id),
                    "comment_added" => {
                        let author = c["author"].as_str().unwrap_or("?");
                        let tag = c["tag"].as_str();
                        let summary = c["body"].as_str().map(first_line).unwrap_or_default();
                        let tag_str = tag.map(|t| format!(" #{}", t)).unwrap_or_default();
                        if summary.is_empty() {
                            println!("  + {}  comment by {}{}", node_id, author, tag_str);
                        } else {
                            println!("  + {}  comment by {}{}: {}", node_id, author, tag_str, summary);
                        }
                    }
                    "comment_removed" => {
                        let ts = c["timestamp"].as_str().unwrap_or("?");
                        println!("  - {}  comment {}", node_id, ts);
                    }
                    "comment_edited" => {
                        let summary = c["body"].as_str().map(first_line).unwrap_or_default();
                        if summary.is_empty() {
                            println!("  ~ {}  comment edited", node_id);
                        } else {
                            println!("  ~ {}  comment edited: {}", node_id, summary);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// Diff a single commit to extract node-level changes.
fn diff_commit(hash: &str, root: &std::path::Path) -> Vec<serde_json::Value> {
    let mut changes = Vec::new();

    // Single diff-tree call against the whole .complex/ directory
    let output = std::process::Command::new("git")
        .args(["diff-tree", "--no-commit-id", "--root", "-r", "--diff-filter=ADMT", hash, "--"])
        .arg(root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return changes,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        // Format: :old_mode new_mode old_hash new_hash status\tpath
        let Some((modes, path)) = line.split_once('\t') else { continue };
        let status = modes.split_whitespace().last().unwrap_or("?");
        let status_char = status.chars().next().unwrap_or('?');

        // Route by path pattern
        if path.contains("/nodes/") && path.ends_with(".json") {
            let node_id = extract_id(path, ".json");
            diff_node_file(&mut changes, hash, path, &node_id, status_char);
        } else if path.contains("/archive/nodes/") && path.ends_with(".json") {
            let node_id = extract_id(path, ".json");
            match status_char {
                'A' => changes.push(serde_json::json!({
                    "node_id": node_id, "action": "archived",
                })),
                'D' => changes.push(serde_json::json!({
                    "node_id": node_id, "action": "unarchived",
                })),
                _ => {}
            }
        } else if path.contains("/issues/") && path.ends_with(".comments.json") {
            let node_id = extract_id(path, ".comments.json");
            diff_comments(&mut changes, hash, path, &node_id, status_char);
        } else if path.contains("/issues/") && path.ends_with(".md") {
            let node_id = extract_id(path, ".md");
            diff_body(&mut changes, hash, path, &node_id, status_char);
        }
    }

    changes
}

/// Extract node ID from a path like ".complex/nodes/lnIl.json" given suffix ".json".
fn extract_id(path: &str, suffix: &str) -> String {
    path.rsplit('/')
        .next()
        .and_then(|f| f.strip_suffix(suffix))
        .unwrap_or("?")
        .to_string()
}

/// Process a changed node file (under nodes/).
fn diff_node_file(
    changes: &mut Vec<serde_json::Value>,
    hash: &str,
    path: &str,
    node_id: &str,
    status: char,
) {
    match status {
        'A' => {
            if let Some(node) = git_show_json(hash, path) {
                let mut c = serde_json::json!({
                    "node_id": node_id,
                    "action": "created",
                    "title": node["title"].as_str().unwrap_or(""),
                    "state": node["state"].as_str().unwrap_or("latent"),
                });
                if let Some(tags) = node["tags"].as_array().filter(|t| !t.is_empty()) {
                    c["tags"] = serde_json::json!(tags);
                }
                changes.push(c);
            } else {
                changes.push(serde_json::json!({
                    "node_id": node_id,
                    "action": "created",
                }));
            }
        }
        'D' => {
            changes.push(serde_json::json!({
                "node_id": node_id,
                "action": "removed",
            }));
        }
        'M' | 'T' => {
            let before = git_show_json(&format!("{}^", hash), path);
            let after = git_show_json(hash, path);
            if let (Some(before), Some(after)) = (before, after) {
                let fields = diff_node_fields(&before, &after);
                if !fields.is_null() {
                    let mut c = serde_json::json!({
                        "node_id": node_id,
                        "action": "modified",
                        "fields": fields,
                    });
                    if let Some(tags) = after["tags"].as_array().filter(|t| !t.is_empty()) {
                        c["tags"] = serde_json::json!(tags);
                    }
                    changes.push(c);
                }
            }
        }
        _ => {}
    }
}

/// Read a JSON file at a specific git revision.
fn git_show_json(rev: &str, path: &str) -> Option<serde_json::Value> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("{}:{}", rev, path)])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

/// Read a text file at a specific git revision.
fn git_show_text(rev: &str, path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("{}:{}", rev, path)])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Diff a body (.md) file change.
fn diff_body(
    changes: &mut Vec<serde_json::Value>,
    hash: &str,
    path: &str,
    node_id: &str,
    status: char,
) {
    match status {
        'A' | 'M' => {
            let action = if status == 'A' { "body_added" } else { "body_edited" };
            let mut c = serde_json::json!({
                "node_id": node_id,
                "action": action,
            });
            if let Some(text) = git_show_text(hash, path) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    c["body"] = serde_json::json!(trimmed);
                }
            }
            changes.push(c);
        }
        'D' => {
            changes.push(serde_json::json!({
                "node_id": node_id,
                "action": "body_removed",
            }));
        }
        _ => {}
    }
}

/// Diff a comments (.comments.json) file change.
fn diff_comments(
    changes: &mut Vec<serde_json::Value>,
    hash: &str,
    path: &str,
    node_id: &str,
    status: char,
) {
    let before: Vec<serde_json::Value> = if status == 'A' {
        vec![]
    } else {
        git_show_json(&format!("{}^", hash), path)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    };

    let after: Vec<serde_json::Value> = if status == 'D' {
        vec![]
    } else {
        git_show_json(hash, path)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    };

    // Index before comments by timestamp for diffing
    let before_by_ts: std::collections::HashMap<&str, &serde_json::Value> = before.iter()
        .filter_map(|c| c["timestamp"].as_str().map(|ts| (ts, c)))
        .collect();
    let after_by_ts: std::collections::HashMap<&str, &serde_json::Value> = after.iter()
        .filter_map(|c| c["timestamp"].as_str().map(|ts| (ts, c)))
        .collect();

    // New comments (in after but not before)
    for c in &after {
        let ts = c["timestamp"].as_str().unwrap_or("");
        if !before_by_ts.contains_key(ts) {
            let mut change = serde_json::json!({
                "node_id": node_id,
                "action": "comment_added",
            });
            if let Some(author) = c["author"].as_str() {
                change["author"] = serde_json::json!(author);
            }
            if let Some(tag) = c["tag"].as_str() {
                change["tag"] = serde_json::json!(tag);
            }
            if let Some(body) = c["body"].as_str() {
                change["body"] = serde_json::json!(body);
            }
            changes.push(change);
        }
    }

    // Removed comments (in before but not after)
    for c in &before {
        let ts = c["timestamp"].as_str().unwrap_or("");
        if !after_by_ts.contains_key(ts) {
            changes.push(serde_json::json!({
                "node_id": node_id,
                "action": "comment_removed",
                "timestamp": ts,
            }));
        }
    }

    // Edited comments (same timestamp, different body)
    for c in &after {
        let ts = c["timestamp"].as_str().unwrap_or("");
        if let Some(prev) = before_by_ts.get(ts)
            && c["body"] != prev["body"]
        {
            let mut change = serde_json::json!({
                "node_id": node_id,
                "action": "comment_edited",
                "timestamp": ts,
            });
            if let Some(body) = c["body"].as_str() {
                change["body"] = serde_json::json!(body);
            }
            changes.push(change);
        }
    }
}

/// First non-empty line, truncated for terminal display.
fn first_line(text: &str) -> String {
    let line = text.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.len() > 80 {
        format!("{}...", &line[..77])
    } else {
        line.to_string()
    }
}

/// Compare two node JSON values and return a map of changed fields.
fn diff_node_fields(before: &serde_json::Value, after: &serde_json::Value) -> serde_json::Value {
    let mut fields = serde_json::Map::new();

    for key in &["state", "title", "part", "shadowed", "parent"] {
        let b = &before[key];
        let a = &after[key];
        if b != a {
            fields.insert(key.to_string(), serde_json::json!({
                "from": display_field(b),
                "to": display_field(a),
            }));
        }
    }

    if fields.is_empty() {
        // Check for edge or tag changes at a coarser level
        if before["edges"] != after["edges"] {
            fields.insert("edges".to_string(), serde_json::json!({
                "from": before["edges"],
                "to": after["edges"],
            }));
        }
        if before["tags"] != after["tags"] {
            fields.insert("tags".to_string(), serde_json::json!({
                "from": before["tags"],
                "to": after["tags"],
            }));
        }
    }

    if fields.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Object(fields)
    }
}

/// Format a JSON value for human display in diffs.
fn display_field(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "∅".to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}
