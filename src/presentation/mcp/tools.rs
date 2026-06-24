use crate::core::antibrick::AntiBrickEngine;
use crate::core::orchestrator::Orchestrator;
use crate::core::watchdog::FilesystemWatchdog;
use crate::domain::*;
use crate::infrastructure::agents::{Agent, AgentRegistry, AgentRole};
use crate::infrastructure::database::Database;
use crate::infrastructure::skills::{Skill, SkillCategory, SkillRegistry};
use serde_json::{json, Value};

use super::html::{
    count_links, derive_encryption_key, extract_headings, extract_links, extract_meta,
    extract_title, format_size2, strip_html,
};

pub fn handle_mem_save(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let title = args["title"].as_str().unwrap_or("Untitled");
    let content = args["content"].as_str().unwrap_or("");
    let obs_type_str = args["type"].as_str().unwrap_or("manual");
    let project = args["project"].as_str().map(|s| s.to_string());
    let scope_str = args["scope"].as_str().unwrap_or("project");
    let session_id = args["session_id"].as_str().unwrap_or("mcp-session");

    let mut obs = Observation::new(
        SessionId::new(session_id),
        obs_type_str
            .parse::<ObservationType>()
            .unwrap_or(ObservationType::Manual),
        title.to_string(),
        content.to_string(),
    );
    obs.project = project;
    obs.scope = if scope_str == "personal" {
        Scope::Personal
    } else {
        Scope::Project
    };

    match db.save_observation(&obs) {
        Ok(id_val) => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Saved: '{}' (id={})", title, id_val) }]
            }
        })),
        Err(e) => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Save failed: {}", e) }]
            }
        })),
    }
}

pub fn handle_mem_search(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let query = args["query"].as_str().unwrap_or("");
    let limit = args["limit"].as_i64().unwrap_or(10) as i32;
    let project = args["project"].as_str();
    let scope = args["scope"].as_str();

    let results = if !query.is_empty() && !query.contains('%') && !query.contains('_') {
        db.search_fts5(query, project, scope, limit)
            .unwrap_or_default()
    } else {
        db.search_fts(query, project, limit).unwrap_or_default()
    };

    let text = if results.is_empty() {
        format!("No results for '{}'", query)
    } else {
        let mut lines = vec![format!("Found {} results for '{}':", results.len(), query)];
        for (i, r) in results.iter().enumerate() {
            let t = r["title"].as_str().unwrap_or("");
            let c = r["content"].as_str().unwrap_or("");
            let preview: String = c.chars().take(200).collect();
            lines.push(format!("\n{}. **{}**\n   {}", i + 1, t, preview));
        }
        lines.join("\n")
    };

    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": text }]
        }
    }))
}

pub fn handle_mem_context(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let limit = args["limit"].as_i64().unwrap_or(5) as i32;
    let project = args["project"].as_str();

    let timeline = db.get_timeline_direct(limit).unwrap_or_default();
    let total = db.stats().unwrap_or_default();
    let default_zero = json!(0);
    let obs = total.get("observations").unwrap_or(&default_zero);

    let mut context_parts = vec![format!(
        "Context\n- Observations: {}\n- Project: {}",
        obs,
        project.unwrap_or("(all)")
    )];

    for (i, entry) in timeline.iter().enumerate() {
        let t = entry.observation.title.as_str();
        let c = entry.observation.content.as_str();
        let preview: String = c.chars().take(150).collect();
        context_parts.push(format!("\n{}. **{}**\n   {}", i + 1, t, preview));
    }

    let text = context_parts.join("\n");

    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": text }]
        }
    }))
}

pub fn handle_mem_timeline(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let limit = args["limit"].as_i64().unwrap_or(20) as i32;

    let results = db.get_timeline_direct(limit).unwrap_or_default();

    let text = if results.is_empty() {
        "No timeline entries found.".to_string()
    } else {
        let mut lines = vec![format!("Timeline (last {}):", results.len())];
        for (i, entry) in results.iter().enumerate() {
            let t = entry.observation.title.as_str();
            let ts = entry.observation.created_at.0;
            lines.push(format!("{}. {} ({})", i + 1, t, ts));
        }
        lines.join("\n")
    };

    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": text }]
        }
    }))
}

pub fn handle_mem_stats(db: &Database, id: &Value) -> anyhow::Result<Value> {
    let stats = db.stats().unwrap_or_default();
    let text = format!(
        "Observations: {}\nActive Agents: {}",
        stats.get("observations").unwrap_or(&json!(0)),
        stats.get("active_agents").unwrap_or(&json!(0))
    );

    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": text }]
        }
    }))
}

pub fn handle_mem_delete(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let obs_id = args["id"].as_i64().unwrap_or(0);
    if obs_id == 0 {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing or invalid observation id" }
        }));
    }
    match db.soft_delete_observation(obs_id) {
        Ok(_) => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Observation {} soft-deleted.", obs_id) }]
            }
        })),
        Err(e) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "result": { "content": [{ "type": "text", "text": format!("Delete failed: {}", e) }] }
        })),
    }
}

pub fn handle_ghost_audit(
    orchestrator: &Orchestrator,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or(".");
    let task_id = orchestrator.create_task(
        &format!("External audit request for {}", path),
        vec!["code_analysis".into()],
        5,
        None,
    );

    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": format!("Audit task created: {}", task_id) }]
        }
    }))
}

pub fn handle_pqc_encrypt(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let plaintext = args["plaintext"].as_str().unwrap_or("");
    let key = derive_encryption_key();
    match crate::core::pqc::encrypt(plaintext.as_bytes(), &key) {
        Ok(ciphertext) => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": hex::encode(ciphertext) }]
            }
        })),
        Err(e) => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Encryption failed: {}", e) }]
            }
        })),
    }
}

pub fn handle_wasm_run(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let wasm_hex = args["wasm_hex"].as_str().unwrap_or("").to_string();
    let entry_func = args["entry_func"].as_str().unwrap_or("main").to_string();

    if wasm_hex.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing required parameter: wasm_hex" }
        }));
    }

    let wasm_size = wasm_hex.len() / 2;
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": format!(
                "WASM module received ({} bytes, entry={}). Execution not yet implemented in this build.",
                wasm_size, entry_func
            )}]
        }
    }))
}

pub fn handle_antibrick_scan(
    engine: &AntiBrickEngine,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let command = args["command"].as_str().unwrap_or("");
    let args_vec: Vec<String> = args["args"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let result =
        crate::core::antibrick::mcp_tools::handle_antibrick_scan(engine, command, args_vec);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": result.to_string() }] }
    }))
}

pub fn handle_antibrick_stats(engine: &AntiBrickEngine, id: &Value) -> anyhow::Result<Value> {
    let stats = crate::core::antibrick::mcp_tools::handle_antibrick_stats(engine);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": stats.to_string() }] }
    }))
}

pub fn handle_antibrick_enable(
    engine: &AntiBrickEngine,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let enable = args["enable"].as_bool().unwrap_or(true);
    let result = crate::core::antibrick::mcp_tools::handle_antibrick_enable(engine, enable);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": result.to_string() }] }
    }))
}

pub fn handle_watchdog_stats(watchdog: &FilesystemWatchdog, id: &Value) -> anyhow::Result<Value> {
    let stats = crate::core::watchdog::mcp_tools::handle_watchdog_stats(watchdog);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": stats.to_string() }] }
    }))
}

pub fn handle_watchdog_verify(watchdog: &FilesystemWatchdog, id: &Value) -> anyhow::Result<Value> {
    let result = crate::core::watchdog::mcp_tools::handle_watchdog_verify(watchdog);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": result.to_string() }] }
    }))
}

pub fn handle_watchdog_snapshot(
    watchdog: &FilesystemWatchdog,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or(".").to_string();
    if path == "/" {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Cannot snapshot entire filesystem. Specify a project path." }
        }));
    }
    let result = crate::core::watchdog::mcp_tools::handle_watchdog_snapshot(watchdog, path);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": result.to_string() }] }
    }))
}

pub fn handle_watchdog_events(
    watchdog: &FilesystemWatchdog,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;
    let result = crate::core::watchdog::mcp_tools::handle_watchdog_events(watchdog, limit);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": result.to_string() }] }
    }))
}

pub fn handle_watchdog_check_path(
    watchdog: &FilesystemWatchdog,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or(".").to_string();
    let result = crate::core::watchdog::mcp_tools::handle_watchdog_check_path(watchdog, path);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": result.to_string() }] }
    }))
}

fn is_private_url(url_str: &str) -> bool {
    let lower = url_str.to_lowercase();
    if lower.contains("localhost") || lower.contains("127.0.0.1") || lower.contains("::1") || lower.contains("[::1]") || lower.contains("0.0.0.0") || lower.contains("169.254.") {
        return true;
    }
    if let Ok(parsed) = url::Url::parse(url_str) {
        if let Some(host) = parsed.host_str() {
            if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "0.0.0.0" {
                return true;
            }
            if host.ends_with(".local") || host.ends_with(".internal") {
                return true;
            }
            if let Some(addr) = host.parse::<std::net::IpAddr>().ok() {
                match addr {
                    std::net::IpAddr::V4(a) => {
                        return a.is_loopback() || a.is_private() || a.is_link_local();
                    }
                    std::net::IpAddr::V6(a) => {
                        return a.is_loopback() || a.is_unicast_link_local();
                    }
                }
            }
        }
    }
    false
}

fn sanitize_path(input: &str) -> std::result::Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(input);
    if path.exists() {
        let canonical = std::fs::canonicalize(path).map_err(|_| format!("Cannot resolve path: {}", input))?;
        return Ok(canonical);
    }
    // File doesn't exist — resolve parent directory
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Err(format!("Parent directory does not exist: {}", parent.display()));
        }
        let canonical_parent = std::fs::canonicalize(parent).map_err(|_| format!("Cannot resolve parent: {}", parent.display()))?;
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let full_path = canonical_parent.join(file_name);
            if full_path.components().any(|c| c.as_os_str() == "..") {
                return Err("Path traversal detected".to_string());
            }
            return Ok(full_path);
        }
    }
    Err(format!("Invalid path: {}", input))
}

pub fn handle_db_backup(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing required parameter: path" }
        }));
    }
    let backup_path = match sanitize_path(path) {
        Ok(p) => p,
        Err(e) => {
            return Ok(json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32602, "message": e }
            }))
        }
    };
    match db.backup_to(&backup_path) {
        Ok(_) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "result": { "content": [{ "type": "text", "text": format!("Database backed up to {}", path) }] }
        })),
        Err(e) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32603, "message": format!("Backup failed: {}", e) }
        })),
    }
}

pub fn handle_db_integrity(db: &Database, id: &Value) -> anyhow::Result<Value> {
    match db.integrity_check() {
        Ok(results) => {
            let ok = results.iter().all(|r| r == "ok");
            let text = if ok {
                "Database integrity: OK".to_string()
            } else {
                format!("Database integrity issues:\n{}", results.join("\n"))
            };
            Ok(json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "content": [{ "type": "text", "text": text }] }
            }))
        }
        Err(e) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32603, "message": format!("Integrity check failed: {}", e) }
        })),
    }
}

pub fn handle_db_prune(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let days = args["older_than_days"].as_i64().unwrap_or(90);
    match db.prune_observations(days) {
        Ok(count) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "result": { "content": [{ "type": "text", "text": format!("Pruned {} observations older than {} days", count, days) }] }
        })),
        Err(e) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32603, "message": format!("Prune failed: {}", e) }
        })),
    }
}

pub fn handle_db_vacuum(db: &Database, id: &Value) -> anyhow::Result<Value> {
    match db.vacuum() {
        Ok(_) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "result": { "content": [{ "type": "text", "text": "Database vacuum completed." }] }
        })),
        Err(e) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32603, "message": format!("Vacuum failed: {}", e) }
        })),
    }
}

pub fn handle_skill_register(
    skills: &SkillRegistry,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let name = args["name"].as_str().unwrap_or("").to_string();
    let description = args["description"].as_str().unwrap_or("").to_string();
    let category_str = args["category"].as_str().unwrap_or("custom");
    if name.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing required parameter: name" }
        }));
    }
    let category = category_str
        .parse::<SkillCategory>()
        .unwrap_or(SkillCategory::Custom);
    let skill = Skill::new(name.clone(), description, category);
    let skill_id = skills.register(skill);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": format!("Skill '{}' registered with id={}", name, skill_id.0) }] }
    }))
}

pub fn handle_skill_list(skills: &SkillRegistry, id: &Value) -> anyhow::Result<Value> {
    let skill_list = skills.list(None);
    let text = if skill_list.is_empty() {
        "No skills registered.".to_string()
    } else {
        let mut lines = vec![format!("Skills ({}):", skill_list.len())];
        for s in &skill_list {
            lines.push(format!("- {} ({})", s.name, s.id.0));
        }
        lines.join("\n")
    };
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": text }] }
    }))
}

pub fn handle_agent_register(
    agents: &AgentRegistry,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let name = args["name"].as_str().unwrap_or("").to_string();
    let role_str = args["role"].as_str().unwrap_or("general");
    let description = args["description"].as_str().unwrap_or("").to_string();
    if name.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing required parameter: name" }
        }));
    }
    let role = role_str.parse::<AgentRole>().unwrap_or(AgentRole::General);
    let agent = Agent::new(name.clone(), role, description);
    let agent_id = agents.register(agent);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": format!("Agent '{}' registered with id={}", name, agent_id.as_str()) }] }
    }))
}

pub fn handle_agent_list(agents: &AgentRegistry, id: &Value) -> anyhow::Result<Value> {
    let agent_list = agents.list(None);
    let text = if agent_list.is_empty() {
        "No agents registered.".to_string()
    } else {
        let mut lines = vec![format!("Agents ({}):", agent_list.len())];
        for a in &agent_list {
            let state_str = format!("{:?}", a.state);
            lines.push(format!("- {} ({:?}) [{}]", a.name, a.role, state_str));
        }
        lines.join("\n")
    };
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": text }] }
    }))
}

pub fn handle_task_create(
    orchestrator: &Orchestrator,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let title = args["title"].as_str().unwrap_or("Untitled").to_string();
    let description = args["description"].as_str().unwrap_or("").to_string();
    let payload = if description.is_empty() {
        title.clone()
    } else {
        description
    };
    let priority = args["priority"].as_i64().unwrap_or(1) as u8;
    let task_id = orchestrator.create_task(&payload, vec!["developer".into()], priority, None);
    let text = format!("Task created: {} (priority={})", task_id, priority);
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": text }] }
    }))
}

pub fn handle_task_list(orchestrator: &Orchestrator, id: &Value) -> anyhow::Result<Value> {
    let tasks = orchestrator.list_tasks();
    let text = if tasks.is_empty() {
        "No tasks.".to_string()
    } else {
        let mut lines = vec![format!("Tasks ({}):", tasks.len())];
        for t in &tasks {
            lines.push(format!("- {} [{}]", t.0, t.1));
        }
        lines.join("\n")
    };
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": text }]
        }
    }))
}

pub fn handle_mcp_call(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let server_url = args["server_url"]
        .as_str()
        .unwrap_or("")
        .trim_end_matches('/');
    let tool_name = args["tool_name"].as_str().unwrap_or("");
    let tool_args = args.get("arguments").cloned().unwrap_or(json!({}));
    let endpoint = args["endpoint"].as_str().unwrap_or("/message");

    if server_url.is_empty() || tool_name.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing required parameters: server_url and tool_name" }
        }));
    }

    static MCP_CALL_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let call_id = MCP_CALL_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let request_body = json!({
        "jsonrpc": "2.0", "id": call_id, "method": "tools/call",
        "params": { "name": tool_name, "arguments": tool_args }
    });

    let url = format!("{}{}", server_url, endpoint);
    if is_private_url(&url) && std::env::var("SYNAPSIS_ALLOW_PRIVATE_MCP").is_err() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": format!("SSRF protection: blocked private URL '{}'. Set SYNAPSIS_ALLOW_PRIVATE_MCP=1 to allow", url) }
        }));
    }
    let mut builder =
        reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(60));
    if std::env::var("SYNAPSIS_INSECURE_TLS").is_ok() {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = builder.build().map_err(|e| e.to_string());

    match client {
        Ok(c) => match c.post(&url).json(&request_body).send() {
            Ok(resp) => {
                let text = resp.text().unwrap_or_default();
                Ok(json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "content": [{ "type": "text", "text": text }] }
                }))
            }
            Err(e) => Ok(json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "content": [{ "type": "text", "text": format!("MCP call failed: {}", e) }] }
            })),
        },
        Err(e) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "result": { "content": [{ "type": "text", "text": format!("HTTP client error: {}", e) }] }
        })),
    }
}

pub fn handle_browser_navigate(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let url = args["url"].as_str().unwrap_or("");
    let method = args["method"].as_str().unwrap_or("GET");
    let extract = args["extract"].as_str().unwrap_or("full");

    if url.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing required parameter: url" }
        }));
    }

    if is_private_url(url) && std::env::var("SYNAPSIS_ALLOW_PRIVATE_MCP").is_err() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": format!("SSRF protection: blocked private URL '{}'. Set SYNAPSIS_ALLOW_PRIVATE_MCP=1 to allow", url) }
        }));
    }

    let mut builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .redirect(reqwest::redirect::Policy::limited(5));
    if std::env::var("SYNAPSIS_INSECURE_TLS").is_ok() {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = builder.build().map_err(|e| e.to_string());

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            return Ok(json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32603, "message": format!("HTTP client error: {}", e) }
            }))
        }
    };

    let response = if method == "POST" {
        let body = args["body"].as_str().unwrap_or("");
        client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body.to_string())
            .send()
    } else {
        client.get(url).send()
    };

    match response {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .to_string();

            let body = resp.text().unwrap_or_default();
            let body_len = body.len();
            if body_len > 10_000_000 {
                return Ok(json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "content": [{ "type": "text", "text": format!("Response too large ({} bytes). Max 10MB.", body_len) }] }
                }));
            }

            let result = match extract {
                "meta" => {
                    let title = extract_title(&body);
                    let desc = extract_meta(&body, "description");
                    let og_title = extract_meta(&body, "og:title");
                    let og_desc = extract_meta(&body, "og:description");
                    let links = count_links(&body);
                    format!(
                        "URL: {}\nStatus: {}\nTitle: {}\nDescription: {}\nOG Title: {}\nOG Desc: {}\nLinks: {}\nSize: {}",
                        url, status, title, desc, og_title, og_desc, links, format_size2(body_len as u64)
                    )
                }
                "links" => {
                    let links = extract_links(&body);
                    let mut out = format!("Links from {} ({}):\n", url, links.len());
                    for (i, link) in links.iter().take(50).enumerate() {
                        out.push_str(&format!("{}. {}\n", i + 1, link));
                    }
                    out
                }
                "text" => {
                    let text = strip_html(&body);
                    let preview: String = text.chars().take(3000).collect();
                    format!("Text from {}:\n{}", url, preview)
                }
                _ => {
                    let title = extract_title(&body);
                    let text = strip_html(&body);
                    let preview: String = text.chars().take(2000).collect();
                    let links = count_links(&body);
                    format!(
                        "URL: {}\nStatus: {}\nTitle: {}\nType: {}\nSize: {}\nLinks: {}\nContent-Type: {}\n\n--- Page ---\n{}",
                        url, status, title, content_type, format_size2(body_len as u64), links, content_type, preview
                    )
                }
            };

            Ok(json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "content": [{ "type": "text", "text": result }] }
            }))
        }
        Err(e) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "result": { "content": [{ "type": "text", "text": format!("Request failed: {}", e) }] }
        })),
    }
}

pub fn handle_browser_snapshot(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let url = args["url"].as_str().unwrap_or("");
    let max_text = args["max_text"].as_u64().unwrap_or(2000) as usize;

    if url.is_empty() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": "Missing required parameter: url" }
        }));
    }

    if is_private_url(url) && std::env::var("SYNAPSIS_ALLOW_PRIVATE_MCP").is_err() {
        return Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32602, "message": format!("SSRF protection: blocked private URL '{}'. Set SYNAPSIS_ALLOW_PRIVATE_MCP=1 to allow", url) }
        }));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| e.to_string());

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            return Ok(json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32603, "message": format!("HTTP client error: {}", e) }
            }))
        }
    };

    match client.get(url).send() {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().unwrap_or_default();
            if body.len() > 10_000_000 {
                return Ok(json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "content": [{ "type": "text", "text": format!("Response too large ({} bytes). Max 10MB.", body.len()) }] }
                }));
            }
            let title = extract_title(&body);
            let desc = extract_meta(&body, "description");
            let text = strip_html(&body);
            let text_preview: String = text.chars().take(max_text).collect();
            let links = count_links(&body);
            let headings = extract_headings(&body);

            let snapshot = format!(
                "Title: {}\nURL: {}\nStatus: {}\nDescription: {}\nHeadings: {}\nLinks: {}\nText ({}):\n{}",
                title, url, status, desc, headings, links, text_preview.len(), text_preview
            );

            Ok(json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "content": [{ "type": "text", "text": snapshot }] }
            }))
        }
        Err(e) => Ok(json!({
            "jsonrpc": "2.0", "id": id,
            "result": { "content": [{ "type": "text", "text": format!("Request failed: {}", e) }] }
        })),
    }
}
