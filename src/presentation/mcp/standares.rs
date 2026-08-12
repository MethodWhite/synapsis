//! Standares resources — port of the standares-mcp Python server.
//!
//! Serves the standards and skills repository at `~/Standares` as MCP
//! resources and tools, following the same pattern as the rest of the
//! Synapsis MCP server. Reads directly from the filesystem; the root can be
//! overridden with `STANDARES_ROOT`.

use anyhow::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const ROOT_ENV: &str = "STANDARES_ROOT";
const SKILLS_SUBDIR: &str = "Skills";
const STANDARDS_SUBDIR: &str = "standards";

fn root_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(ROOT_ENV) {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Standares")
}

/// Walk a directory collecting `.md` files as `(absolute, relative)` paths.
fn walk_md(root: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    fn recurse(dir: &Path, root: &Path, out: &mut Vec<(PathBuf, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                recurse(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push((path.clone(), rel.to_string_lossy().replace('\\', "/")));
                }
            }
        }
    }
    recurse(root, root, &mut out);
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StandaresItem {
    pub uri: String,
    pub name: String,
    pub domain: String,
    pub path: String,
}

/// Build the full inventory of skills and standards.
fn inventory() -> Vec<StandaresItem> {
    let root = root_dir();
    let mut items = Vec::new();

    let skills_root = root.join(SKILLS_SUBDIR);
    if skills_root.is_dir() {
        for (full, rel) in walk_md(&skills_root) {
            let parts: Vec<&str> = rel.split('/').collect();
            if parts.len() < 2 {
                continue;
            }
            let fname = parts.last().unwrap_or(&"").trim_end_matches(".md");
            let sub = if parts.len() >= 3 { parts[parts.len() - 2] } else { "" };
            let name = if fname == "skill" || fname == "skills" {
                sub.to_string()
            } else {
                fname.to_string()
            };
            let uri_name = name.clone();
            let domain = parts[0].to_string();
            let _ = full;
            items.push(StandaresItem {
                uri: format!("standares://skills/{domain}/{uri_name}"),
                name,
                domain,
                path: rel,
            });
        }
    }

    let std_root = root.join(STANDARDS_SUBDIR);
    if std_root.is_dir() {
        let mut seen = std::collections::HashSet::new();
        // Walk the standards root so relative paths include the category
        // directory (e.g. `testing-quality/testing-quality.md`).
        let all = walk_md(&std_root);
        for (_full, r) in &all {
            let parts: Vec<&str> = r.split('/').collect();
            let Some(cat) = parts.first().map(|s| s.to_string()) else {
                continue;
            };
            if seen.contains(&cat) {
                continue;
            }
            seen.insert(cat.clone());
            let chosen = all
                .iter()
                .filter(|(_, rr)| rr.starts_with(&format!("{cat}/")))
                .find(|(f, _)| {
                    let stem = f
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    stem == cat.to_lowercase() || stem == cat.to_lowercase().replace('-', "_")
                })
                .map(|(_, rr)| rr.clone())
                .or_else(|| {
                    all.iter()
                        .find(|(_, rr)| rr.starts_with(&format!("{cat}/")))
                        .map(|(_, rr)| rr.clone())
                });
            if let Some(path) = chosen {
                items.push(StandaresItem {
                    uri: format!("standares://standards/{cat}"),
                    name: cat.clone(),
                    domain: "standards".to_string(),
                    path,
                });
            }
        }
    }

    items
}

fn item_path(root: &Path, item: &StandaresItem) -> PathBuf {
    let base = if item.domain == "standards" {
        root.join(STANDARDS_SUBDIR)
    } else {
        root.join(SKILLS_SUBDIR)
    };
    base.join(&item.path)
}

/// Read the raw markdown content for a `standares://` URI.
fn read_uri(uri: &str) -> Option<String> {
    let root = root_dir();
    for item in inventory() {
        if item.uri == uri {
            let full = item_path(&root, &item);
            return std::fs::read_to_string(&full).ok();
        }
    }
    None
}

/// Full-text search over the repository (name, domain and content).
fn search(q: &str) -> Vec<StandaresItem> {
    let ql = q.to_lowercase();
    let root = root_dir();
    inventory()
        .into_iter()
        .filter(|item| {
            if item.name.to_lowercase().contains(&ql) || item.domain.to_lowercase().contains(&ql) {
                return true;
            }
            let full = item_path(&root, item);
            match std::fs::read_to_string(&full) {
                Ok(text) => text.to_lowercase().contains(&ql),
                Err(_) => false,
            }
        })
        .take(50)
        .collect()
}

fn stats() -> Value {
    let root = root_dir();
    let skills = walk_md(&root.join(SKILLS_SUBDIR)).len();
    let standards = walk_md(&root.join(STANDARDS_SUBDIR)).len();
    let pdfs = walk_dir_count(&root, "pdf");
    json!({ "skills_md": skills, "standards_md": standards, "pdfs": pdfs })
}

fn walk_dir_count(root: &Path, ext: &str) -> usize {
    fn recurse(dir: &Path, ext: &str, out: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                recurse(&path, ext, out);
            } else if path.extension().is_some_and(|e| e == ext) {
                *out += 1;
            }
        }
    }
    let mut n = 0;
    recurse(root, ext, &mut n);
    n
}

/// `resources/list` — synapsis resources plus the standares repository.
pub fn list_resources() -> Vec<Value> {
    let mut out = vec![
        json!({"uri": "synapsis://memory", "name": "Synapsis Memory"}),
        json!({"uri": "synapsis://skills", "name": "Synapsis Skills"}),
        json!({"uri": "synapsis://agents", "name": "Synapsis Agents"}),
    ];
    for item in inventory() {
        out.push(json!({
            "uri": item.uri,
            "name": format!("{}/{}", item.domain, item.name),
            "description": item.path,
        }));
    }
    out
}

/// `resources/read` for standares URIs. Returns `None` when the URI is not
/// a standares resource (the caller decides the fallback response).
pub fn read_resource(uri: &str) -> Option<Value> {
    let contents = match uri {
        "standares://list" => json!(inventory()),
        "standares://stats" => stats(),
        "standares://search" => json!(search(
            uri.strip_prefix("standares://search?q=").unwrap_or("")
        )),
        _ => {
            if uri.starts_with("standares://") {
                let text = read_uri(uri)?;
                return Some(json!({ "contents": [{ "uri": uri, "text": text }] }));
            }
            return None;
        }
    };
    Some(json!({ "contents": [{ "uri": uri, "text": json!(contents).to_string() }] }))
}

pub fn handle_standares_search(id: &Value, args: &Value) -> Result<Value> {
    let q = args["query"].as_str().unwrap_or("");
    let hits: Vec<Value> = search(q)
        .into_iter()
        .map(|h| json!({"uri": h.uri, "name": h.name, "domain": h.domain}))
        .collect();
    Ok(json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "content": [{ "type": "text", "text": json!(hits).to_string() }] }
    }))
}

pub fn handle_standares_stats(id: &Value) -> Result<Value> {
    Ok(json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "content": [{ "type": "text", "text": stats().to_string() }] }
    }))
}

pub fn handle_standares_list(id: &Value) -> Result<Value> {
    Ok(json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "content": [{ "type": "text", "text": json!(inventory()).to_string() }] }
    }))
}
