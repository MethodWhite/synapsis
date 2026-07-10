use crate::core::antibrick::AntiBrickEngine;
use crate::core::auth::permissions::{Permission, PermissionSet};
use crate::core::orchestrator::Orchestrator;
use crate::core::watchdog::FilesystemWatchdog;
use crate::domain::*;
use crate::infrastructure::agents::{Agent, AgentRegistry, AgentRole};
use crate::infrastructure::database::Database;
use crate::infrastructure::skills::{Skill, SkillCategory, SkillRegistry};
use serde_json::{Value, json};

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

pub fn handle_mem_timeline(
    timelines: &crate::core::timeline_manager::TimelineManager,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let limit = args["limit"].as_i64().unwrap_or(20) as i32;

    match timelines.get_timeline(limit) {
        Ok(results) => {
            let text = if results.is_empty() {
                "No timeline entries found.".to_string()
            } else {
                let mut lines = vec![format!("Timeline (last {}):", results.len())];
                for (i, entry) in results.iter().enumerate() {
                    lines.push(format!(
                        "{}. {} ({}) [{}]",
                        i + 1,
                        entry.title,
                        entry.created_at,
                        entry.observation_type
                    ));
                }
                lines.join("\n")
            };
            Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
        }
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
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
    let agent_id = args["agent_id"].as_str();
    let session_id = args["session_id"].as_str();
    match db.soft_delete_observation(obs_id) {
        Ok(_) => {
            db.log_audit(
                "delete",
                Some(obs_id),
                agent_id,
                session_id,
                None,
                None,
                args["reason"].as_str(),
            )
            .ok();
            Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": format!("Observation {} soft-deleted.", obs_id) }]
                }
            }))
        }
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
    if lower.contains("localhost")
        || lower.contains("127.0.0.1")
        || lower.contains("::1")
        || lower.contains("[::1]")
        || lower.contains("0.0.0.0")
        || lower.contains("169.254.")
    {
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
        let canonical =
            std::fs::canonicalize(path).map_err(|_| format!("Cannot resolve path: {}", input))?;
        return Ok(canonical);
    }
    // File doesn't exist — resolve parent directory
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Err(format!(
                "Parent directory does not exist: {}",
                parent.display()
            ));
        }
        let canonical_parent = std::fs::canonicalize(parent)
            .map_err(|_| format!("Cannot resolve parent: {}", parent.display()))?;
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
            }));
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

pub fn handle_mem_update(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let obs_id = args["id"].as_i64().unwrap_or(0);
    let title = args["title"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if obs_id == 0 {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'id'"}}),
        );
    }
    match db.update_observation(obs_id, title, content) {
        Ok(_) => {
            db.log_audit(
                "update",
                Some(obs_id),
                args["agent_id"].as_str(),
                args["session_id"].as_str(),
                None,
                Some(content),
                args["reason"].as_str(),
            )
            .ok();
            Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Observation {} updated.",obs_id)}]}}),
            )
        }
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_get_observation(
    db: &Database,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let obs_id = args["id"].as_i64().unwrap_or(0);
    if obs_id == 0 {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'id'"}}),
        );
    }
    match db.get_observation_by_id(obs_id) {
        Ok(Some(obs)) => {
            let text = format!(
                "# {}\n\n{}\n\nProject: {} | Type: {} | Created: {}",
                obs["title"].as_str().unwrap_or(""),
                obs["content"].as_str().unwrap_or(""),
                obs["project"].as_str().unwrap_or("(none)"),
                obs["observation_type"].as_i64().unwrap_or(0),
                obs["created_at"].as_i64().unwrap_or(0),
            );
            Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
        }
        Ok(None) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":"Observation not found."}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_judge(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let source_id = args["source_id"].as_i64().unwrap_or(0);
    let target_id = args["target_id"].as_i64().unwrap_or(0);
    let relation = args["relation"].as_str().unwrap_or("");
    let reason = args["reason"].as_str();
    let evidence = args["evidence"].as_str();
    let confidence = args["confidence"].as_f64().unwrap_or(1.0).clamp(0.0, 1.0);
    let session_id = args["session_id"].as_str();
    let project = args["project"].as_str();
    let valid_relations = [
        "related",
        "compatible",
        "scoped",
        "conflicts_with",
        "supersedes",
        "not_conflict",
    ];
    if source_id == 0 || target_id == 0 || !valid_relations.contains(&relation) {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Invalid params: source_id, target_id, and relation required"}}),
        );
    }
    if relation == "not_conflict" {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":"No conflict recorded (not_conflict is a no-op)."}]}}),
        );
    }
    match db.insert_relation(
        source_id, target_id, relation, reason, evidence, confidence, session_id, project,
    ) {
        Ok(sync_id) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Relation '{}' recorded (sync_id={})", relation, sync_id)}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_compare(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let memory_id_a = args["memory_id_a"].as_i64().unwrap_or(0);
    let memory_id_b = args["memory_id_b"].as_i64().unwrap_or(0);
    let relation = args["relation"].as_str().unwrap_or("");
    let confidence = args["confidence"].as_f64().unwrap_or(1.0).clamp(0.0, 1.0);
    let reasoning = args["reasoning"].as_str().unwrap_or("");
    let model = args["model"].as_str();
    let valid_relations = [
        "related",
        "compatible",
        "scoped",
        "conflicts_with",
        "supersedes",
        "not_conflict",
    ];
    if memory_id_a == 0 || memory_id_b == 0 || !valid_relations.contains(&relation) {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Invalid params"}}),
        );
    }
    if relation == "not_conflict" {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":"No conflict (not_conflict)."}]}}),
        );
    }
    match db.insert_relation(
        memory_id_a,
        memory_id_b,
        relation,
        Some(reasoning),
        model,
        confidence,
        None,
        None,
    ) {
        Ok(sync_id) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Comparison '{}' recorded (sync_id={})", relation, sync_id)}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_session_start(
    sessions: &crate::core::session_manager::SessionManager,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let project = args["project"].as_str().unwrap_or("default");
    let directory = args["directory"].as_str().unwrap_or(".");
    match sessions.start_session(project, directory) {
        Ok(sid) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Session started: {}", sid)}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_session_end(
    sessions: &crate::core::session_manager::SessionManager,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let session_id = args["session_id"].as_str().unwrap_or("");
    let summary = args["summary"].as_str();
    if session_id.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'session_id'"}}),
        );
    }
    match sessions.end_session(session_id, summary) {
        Ok(_) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Session {} ended.", session_id)}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_session_summary(
    sessions: &crate::core::session_manager::SessionManager,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let session_id = args["session_id"].as_str().unwrap_or("mcp-session");
    match sessions.generate_summary(session_id) {
        Ok(summary) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":summary}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_doctor(db: &Database, id: &Value) -> anyhow::Result<Value> {
    match db.doctor_check() {
        Ok(diag) => {
            let text = format!(
                "Synapsis Memory Diagnostics\nStatus: {}\nObservations: {}\nDeleted: {}\nSessions: {}\nFTS entries: {}\nZero-hash observations: {}\nBroken relations: {}",
                diag["status"].as_str().unwrap_or("unknown"),
                diag["observations"].as_i64().unwrap_or(-1),
                diag["deleted"].as_i64().unwrap_or(-1),
                diag["sessions"].as_i64().unwrap_or(-1),
                diag["fts_entries"].as_i64().unwrap_or(-1),
                diag["zero_hash_observations"].as_i64().unwrap_or(-1),
                diag["broken_relations"].as_i64().unwrap_or(-1),
            );
            Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
        }
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_merge_projects(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let source = args["source"].as_str().unwrap_or("");
    let target = args["target"].as_str().unwrap_or("");
    if source.is_empty() || target.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'source' and 'target'"}}),
        );
    }
    match db.merge_projects(source, target) {
        Ok(count) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Merged {} observations from '{}' into '{}'", count, source, target)}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_current_project(
    db: &Database,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let project = args["project"].as_str().unwrap_or("");
    if project.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'project'"}}),
        );
    }
    let count: i64 = db
        .get_conn()
        .query_row(
            "SELECT COUNT(*) FROM observations WHERE project = ?1 AND deleted_at IS NULL",
            rusqlite::params![project],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let text = format!("Project: {} | Active observations: {}", project, count);
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_mem_audit_log(db: &Database, id: &Value, args: &Value) -> anyhow::Result<Value> {
    let limit = args["limit"].as_i64().unwrap_or(20) as i32;
    match db.get_audit_trail(limit) {
        Ok(entries) => {
            let text = if entries.is_empty() {
                "No audit entries.".to_string()
            } else {
                let mut lines = vec![format!("Audit log (last {}):", entries.len())];
                for e in &entries {
                    lines.push(format!(
                        "[{}] {} (obs#{}) by {} -- {}",
                        e["created_at"].as_i64().unwrap_or(0),
                        e["action"].as_str().unwrap_or("?"),
                        e["observation_id"].as_i64().unwrap_or(0),
                        e["agent_id"].as_str().unwrap_or("?"),
                        e["reason"].as_str().unwrap_or(""),
                    ));
                }
                lines.join("\n")
            };
            Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
        }
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_mem_recycle_save(
    recycle: &crate::core::recycle::RecycleBin,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let content = args["content"].as_str().unwrap_or("");
    match recycle.store(content.as_bytes(), None, "mcp") {
        Ok(entry_id) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Recycled (id={})", entry_id)}]}}),
        ),
        Err(e) => Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":format!("Recycle failed: {:?}", e)}}),
        ),
    }
}

pub fn handle_mem_recycle_search(
    recycle: &crate::core::recycle::RecycleBin,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let keyword = args["keyword"].as_str().unwrap_or("");
    let limit = args["limit"].as_i64().unwrap_or(20) as usize;
    let query = crate::core::recycle::SearchQuery {
        keywords: if keyword.is_empty() {
            vec![]
        } else {
            vec![keyword.to_string()]
        },
        category: None,
        limit,
        offset: args["offset"].as_i64().unwrap_or(0) as usize,
        agent_fingerprint: None,
        task_id: None,
        from_time: None,
        to_time: None,
    };
    let results = recycle.search(&query);
    let text = if results.entries.is_empty() {
        "No recycled items found.".to_string()
    } else {
        let mut lines = vec![format!("Recycled items ({}):", results.total)];
        for e in &results.entries {
            lines.push(format!("[{}] size={}", e.id, e.size_bytes));
        }
        lines.join("\n")
    };
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_mem_recycle_stats(
    recycle: &crate::core::recycle::RecycleBin,
    id: &Value,
) -> anyhow::Result<Value> {
    let stats = recycle.stats();
    let text = format!(
        "Recycle Bin: {} items, {} bytes",
        stats.total_entries, stats.total_size_bytes
    );
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_mem_recycle_delete(
    recycle: &crate::core::recycle::RecycleBin,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let entry_id = args["id"].as_str().unwrap_or("");
    if entry_id.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'id'"}}),
        );
    }
    if recycle.delete(entry_id) {
        let _ = recycle.save();
        Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Deleted recycled entry {}", entry_id)}]}}),
        )
    } else {
        Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":"Entry not found"}}))
    }
}

pub fn handle_agent_unregister(
    agent_ext: &crate::core::agent_registry_ext::AgentRegistryExt,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let agent_id = args["agent_id"].as_str().unwrap_or("");
    if agent_id.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'agent_id'"}}),
        );
    }
    match agent_ext.unregister_agent(agent_id) {
        Ok(_) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Agent {} unregistered.", agent_id)}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_agent_list_by_project(
    agent_ext: &crate::core::agent_registry_ext::AgentRegistryExt,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let project = args["project"].as_str().unwrap_or("");
    if project.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'project'"}}),
        );
    }
    match agent_ext.list_agents_by_project(project) {
        Ok(agents) => {
            let text = if agents.is_empty() {
                format!("No agents in project '{}'.", project)
            } else {
                let mut lines = vec![format!("Agents in '{}' ({}):", project, agents.len())];
                for a in &agents {
                    let status = if a.is_active { "active" } else { "inactive" };
                    lines.push(format!(
                        "- {} ({}) [{}] last: {}",
                        a.id, a.agent_type, status, a.last_heartbeat
                    ));
                }
                lines.join("\n")
            };
            Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
        }
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_chunk_query(
    chunks: &crate::core::chunk_query::ChunkQueryManager,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let chunk_id = args["chunk_id"].as_str().unwrap_or("");
    let project = args["project"].as_str().unwrap_or("");
    if !chunk_id.is_empty() {
        match chunks.get_chunk(chunk_id) {
            Ok(Some(chunk)) => Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Chunk: {}\nProject: {}\nTitle: {}\nSize: {} bytes\nCreated: {}", chunk.chunk_id, chunk.project_key, chunk.title, chunk.content.len(), chunk.created_at)}]}}),
            ),
            Ok(None) => Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Chunk {} not found.", chunk_id)}]}}),
            ),
            Err(e) => {
                Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
            }
        }
    } else if !project.is_empty() {
        match chunks.get_chunks_by_project(project, None) {
            Ok(chunks_list) => {
                let text = if chunks_list.is_empty() {
                    format!("No chunks in project '{}'.", project)
                } else {
                    let mut lines =
                        vec![format!("Chunks in '{}' ({}):", project, chunks_list.len())];
                    for c in &chunks_list {
                        lines.push(format!(
                            "- {} ({}) {} bytes",
                            c.chunk_id,
                            c.title,
                            c.content.len()
                        ));
                    }
                    lines.join("\n")
                };
                Ok(
                    json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}),
                )
            }
            Err(e) => {
                Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
            }
        }
    } else {
        Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'chunk_id' or 'project'"}}),
        )
    }
}

pub fn handle_vault_store(
    vault: &crate::core::vault::SecureVault,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let key = args["key"].as_str().unwrap_or("");
    let value = args["value"].as_str().unwrap_or("");
    if key.is_empty() || value.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'key' and 'value'"}}),
        );
    }
    match vault.store_secret(key, value) {
        Ok(_) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Stored secret under key '{}'.", key)}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_vault_retrieve(
    vault: &crate::core::vault::SecureVault,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let key = args["key"].as_str().unwrap_or("");
    if key.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'key'"}}),
        );
    }
    match vault.retrieve_secret(key) {
        Ok(data) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":data}]}}))
        }
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_worker_execute(
    workers: &crate::core::worker::WorkerOrchestrator,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let task_type_str = args["task_type"].as_str().unwrap_or("");
    let command = args["command"].as_str().unwrap_or("");
    if task_type_str.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'task_type' (shell, file_read, file_write, code_analysis, code_refactor, search, git)"}}),
        );
    }
    let task_type = match task_type_str {
        "shell" => crate::core::worker::TaskType::Shell,
        "file_read" => crate::core::worker::TaskType::FileRead,
        "file_write" => crate::core::worker::TaskType::FileWrite,
        "code_analysis" => crate::core::worker::TaskType::CodeAnalysis,
        "code_refactor" => crate::core::worker::TaskType::CodeRefactor,
        "search" => crate::core::worker::TaskType::Search,
        "git" => crate::core::worker::TaskType::Git,
        _ => {
            return Ok(
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":format!("Unknown task_type: {}", task_type_str)}}),
            );
        }
    };
    let mut payload = serde_json::json!({});
    if !command.is_empty() {
        payload["command"] = serde_json::json!(command);
    }
    if let Some(path) = args["path"].as_str() {
        payload["path"] = serde_json::json!(path);
    }
    if let Some(query) = args["query"].as_str() {
        payload["query"] = serde_json::json!(query);
    }
    let skill = args["skill"].as_str().unwrap_or(task_type_str);
    let task =
        crate::core::worker::Task::new(task_type, payload).with_skills(vec![skill.to_string()]);
    match workers.execute_task(task) {
        Ok(result) => {
            let status = match result.status {
                crate::core::worker::TaskStatus::Success => "success",
                crate::core::worker::TaskStatus::Failed => "failed",
                crate::core::worker::TaskStatus::Timeout => "timeout",
                crate::core::worker::TaskStatus::Cancelled => "cancelled",
            };
            Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Status: {}\nDuration: {}ms\nOutput: {}", status, result.duration_ms, result.output)}]}}),
            )
        }
        Err(e) => Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":format!("[0x{:04X}] {}", e.code, e.message)}}),
        ),
    }
}

pub fn handle_worker_status(
    workers: &crate::core::worker::WorkerOrchestrator,
    id: &Value,
) -> anyhow::Result<Value> {
    let status = workers.get_status();
    Ok(
        json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Workers: {:?}\nExternal: {:?}", status["builtin_workers"], status["external_agents"])}]}}),
    )
}

pub fn handle_sync_status(
    git_sync: &crate::core::sync::GitSyncEngine,
    id: &Value,
) -> anyhow::Result<Value> {
    let status = git_sync.get_sync_status();
    let text = format!(
        "Last sync: {}\nCommits: {}\nPending: {}\nConflicts: {}\nCircuit: {}",
        status.last_sync,
        status.commit_count,
        status.pending_changes,
        status.conflict_detected,
        status.circuit_breaker_state
    );
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_auto_discover(
    auto_int: &crate::core::auto_integrate::AutoIntegrate,
    id: &Value,
) -> anyhow::Result<Value> {
    let scan = auto_int.force_scan();
    let text = format!(
        "Discovery scan: {} total found, {} new ({}ms)\nBy type: {:?}",
        scan.total_found,
        scan.new_tools.len(),
        scan.scan_time_ms,
        scan.by_type
    );
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_auth_tpm_status(
    tpm: &crate::core::auth::tpm::TpmMfaProvider,
    id: &Value,
) -> anyhow::Result<Value> {
    let available = tpm.is_tpm_available();
    let text = format!("TPM availability: {:?}", available);
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_auth_tpm_attest(
    tpm: &crate::core::auth::tpm::TpmMfaProvider,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let nonce = args["nonce"].as_str().unwrap_or("");
    if nonce.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'nonce'"}}),
        );
    }
    match tpm.generate_tpm_attestation(nonce) {
        Ok(att) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("TPM attestation generated for nonce: {}\nDevice: {:?}", nonce, att)}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_auth_check_permission(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let perm_str = args["permission"].as_str().unwrap_or("");
    let trust_str = args["trust_level"].as_str().unwrap_or("basic");
    if perm_str.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'permission'"}}),
        );
    }
    let perms = match trust_str {
        "all" => PermissionSet::all(),
        "trusted" => PermissionSet::trusted(),
        "basic" => PermissionSet::basic(),
        "minimal" => PermissionSet::minimal(),
        _ => PermissionSet::none(),
    };
    let perm = match perm_str {
        "pqc_encrypt" => Permission::PqcEncrypt,
        "pqc_decrypt" => Permission::PqcDecrypt,
        "manage_agents" => Permission::ManageAgents,
        "configure_security" => Permission::ConfigureSecurity,
        "read_recycle" => Permission::ReadRecycleBin,
        "write_recycle" => Permission::WriteRecycleBin,
        "admin" => Permission::Admin,
        "create_task" => Permission::CreateTask,
        "execute_task" => Permission::ExecuteTask,
        _ => {
            return Ok(
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":format!("Unknown permission: {}", perm_str)}}),
            );
        }
    };
    let allowed = perms.has_permission(perm);
    Ok(
        json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Permission '{}' for trust level '{}': {}", perm_str, trust_str, if allowed { "ALLOWED" } else { "DENIED" })}]}}),
    )
}

pub fn handle_auth_classify_agent(
    classifier: &crate::core::auth::classifier::AgentClassifier,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let agent_type = args["agent_type"].as_str().unwrap_or("");
    if agent_type.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'agent_type'"}}),
        );
    }
    let client_type = crate::core::auth::classifier::ClientType::from_agent_type(agent_type);
    let config = classifier.get_config();
    let text = format!(
        "Agent: {}\nClient type: {:?}\nConfig: {:?}",
        agent_type, client_type, config
    );
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_secure_write_file(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or("");
    let data = args["data"].as_str().unwrap_or("");
    if path.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'path' and 'data'"}}),
        );
    }
    match crate::core::security::SysCall::write_file(path, data.as_bytes()) {
        Ok(_) => Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Written {} bytes to {}", data.len(), path)}]}}),
        ),
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_secure_read_file(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'path'"}}),
        );
    }
    match crate::core::security::SysCall::read_file(path) {
        Ok(data) => {
            let text = String::from_utf8_lossy(&data).to_string();
            Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
        }
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_secure_list_dir(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let path = args["path"].as_str().unwrap_or(".");
    match crate::core::security::SysCall::list_directory(path) {
        Ok(entries) => {
            let text = if entries.is_empty() {
                format!("Directory '{}' is empty.", path)
            } else {
                format!(
                    "Directory '{}' ({} entries):\n{}",
                    path,
                    entries.len(),
                    entries.join("\n")
                )
            };
            Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
        }
        Err(e) => {
            Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
        }
    }
}

pub fn handle_secure_random(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let max = args["max"].as_i64().unwrap_or(1000) as u64;
    let val = crate::core::security::secure_random_u64() % max;
    Ok(
        json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":val.to_string()}]}}),
    )
}

pub fn handle_vault_session_key(
    vault: &crate::core::vault::SecureVault,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let session_id = args["session_id"].as_str().unwrap_or("");
    let action = args["action"].as_str().unwrap_or("get");
    if session_id.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'session_id'"}}),
        );
    }
    match action {
        "get" => match vault.get_session_key(session_id) {
            Ok(Some(key)) => Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Session key for {}: expires at {:?}", session_id, key.expires_at)}]}}),
            ),
            Ok(None) => Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("No session key for {}", session_id)}]}}),
            ),
            Err(e) => {
                Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
            }
        },
        "rotate" => match vault.rotate_key(session_id) {
            Ok(Some(new_key)) => Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Key rotated for {}: new key prefix: {}...", session_id, &new_key[..new_key.len().min(8)])}]}}),
            ),
            Ok(None) => Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("No session key to rotate for {}", session_id)}]}}),
            ),
            Err(e) => {
                Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
            }
        },
        "close" => {
            vault.close_session(session_id);
            Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Session {} closed.", session_id)}]}}),
            )
        }
        _ => Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":format!("Unknown action: {}", action)}}),
        ),
    }
}

pub fn handle_vault_list_sessions(
    vault: &crate::core::vault::SecureVault,
    id: &Value,
) -> anyhow::Result<Value> {
    let sessions = vault.list_sessions();
    let text = if sessions.is_empty() {
        "No active session keys.".to_string()
    } else {
        let mut lines = vec![format!("Active session keys ({}):", sessions.len())];
        for (sid, key_prefix, ts) in &sessions {
            lines.push(format!(
                "- {} (key: {}... created: {})",
                sid,
                &key_prefix[..key_prefix.len().min(8)],
                ts
            ));
        }
        lines.join("\n")
    };
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_sync_memory(
    git_sync: &crate::core::sync::GitSyncEngine,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let agent_id = args["agent_id"].as_str().unwrap_or("default");
    if let Some(summary) = args["summary"].as_str() {
        let memory = crate::domain::entities::Memory {
            id: crate::core::uuid::Uuid::new_v4().to_hex_string(),
            agent_id: agent_id.to_string(),
            session_id: None,
            role: "sync".to_string(),
            content: summary.to_string(),
            token_count: summary.len() as i32,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            checksum: None,
        };
        match git_sync.sync_memory(&memory) {
            Ok(manifest_id) => Ok(
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Memory synced. Manifest: {}", manifest_id.0)}]}}),
            ),
            Err(e) => {
                Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
            }
        }
    } else {
        Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'summary'"}}))
    }
}

pub fn handle_audit_log(id: &Value, args: &Value) -> anyhow::Result<Value> {
    let obs_id = args["observation_id"].as_i64().unwrap_or(0);
    let log = crate::core::audit_log::AuditLog::new();
    if obs_id > 0 {
        match log.get_audit_trail(obs_id) {
            Ok(entries) => {
                let text = if entries.is_empty() {
                    format!("No audit entries for observation {}.", obs_id)
                } else {
                    let mut lines = vec![format!(
                        "Audit trail for observation {} ({} entries):",
                        obs_id,
                        entries.len()
                    )];
                    for e in &entries {
                        lines.push(format!(
                            "- {} [{}] {} obs:{}",
                            e.timestamp, e.agent_id, e.action, e.observation_id
                        ));
                    }
                    lines.join("\n")
                };
                Ok(
                    json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}),
                )
            }
            Err(e) => {
                Ok(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}))
            }
        }
    } else {
        Ok(
            json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":"Provide 'observation_id' to get audit trail."}]}}),
        )
    }
}

pub fn handle_resource_stats(
    resources: &crate::core::resource_manager::ResourceManager,
    id: &Value,
) -> anyhow::Result<Value> {
    let stats = resources.get_system_stats();
    let text = format!(
        "System stats:\n  CPU: {:.1}%\n  Memory: {:.1}%\n  Load (1/5/15): {:.2}/{:.2}/{:.2}\n  Swap: {} / {} bytes\n  Throttle delay: {}ms",
        stats.cpu_usage_percent,
        stats.memory_usage_percent,
        stats.load_average_1min,
        stats.load_average_5min,
        stats.load_average_15min,
        stats.used_swap_bytes,
        stats.total_swap_bytes,
        resources.get_throttle_delay_ms()
    );
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_resource_recommendations(
    resources: &crate::core::resource_manager::ResourceManager,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let agent_id = args["agent_id"].as_str().unwrap_or("");
    if agent_id.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'agent_id'"}}),
        );
    }
    let recs = resources.get_agent_recommendations(agent_id);
    let text = format!(
        "Recommendations for '{}':\n  Max tasks: {}\n  Current tasks: {}\n  Should throttle: {}\n  Throttle delay: {}ms",
        agent_id,
        recs.recommended_max_tasks,
        recs.current_tasks,
        recs.should_throttle,
        recs.throttle_delay_ms
    );
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_orchestrator_tree(
    orchestrator: &crate::core::orchestrator::Orchestrator,
    id: &Value,
    args: &Value,
) -> anyhow::Result<Value> {
    let agent_id = args["agent_id"].as_str().unwrap_or("");
    if agent_id.is_empty() {
        return Ok(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Missing 'agent_id'"}}),
        );
    }
    let tree = orchestrator.get_sub_agent_tree(agent_id);
    let text = if tree.is_empty() {
        format!("No sub-agents for '{}'.", agent_id)
    } else {
        let mut lines = vec![format!(
            "Sub-agent tree for '{}' ({}):",
            agent_id,
            tree.len()
        )];
        for agent in &tree {
            lines.push(format!(
                "- {} ({}) skills: {:?}",
                agent.name, agent.agent_type, agent.skills
            ));
        }
        lines.join("\n")
    };
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
}

pub fn handle_orchestrator_idle(
    orchestrator: &crate::core::orchestrator::Orchestrator,
    id: &Value,
) -> anyhow::Result<Value> {
    let idle = orchestrator.get_idle_agents();
    let text = if idle.is_empty() {
        "No idle agents.".to_string()
    } else {
        let mut lines = vec![format!("Idle agents ({}):", idle.len())];
        for agent in &idle {
            lines.push(format!("- {} ({})", agent.name, agent.agent_type));
        }
        lines.join("\n")
    };
    Ok(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}}))
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
            }));
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
                        url,
                        status,
                        title,
                        desc,
                        og_title,
                        og_desc,
                        links,
                        format_size2(body_len as u64)
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
                        url,
                        status,
                        title,
                        content_type,
                        format_size2(body_len as u64),
                        links,
                        content_type,
                        preview
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
            }));
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
                title,
                url,
                status,
                desc,
                headings,
                links,
                text_preview.len(),
                text_preview
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
