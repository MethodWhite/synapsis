use crate::core::lock_utils::*;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use crate::core::antibrick::{AntiBrickConfig, AntiBrickEngine};
use crate::core::orchestrator::Orchestrator;
use crate::core::watchdog::FilesystemWatchdog;
use crate::domain::*;
use crate::infrastructure::agents::AgentRegistry;
use crate::infrastructure::database::Database;
use crate::infrastructure::skills::SkillRegistry;

use super::html::format_args_snapshot;
use super::tools;

macro_rules! info_log {
    ($($arg:tt)*) => {{
        if !crate::config::is_quiet() {
            eprintln!($($arg)*);
        }
    }};
}

macro_rules! debug_log {
    ($($arg:tt)*) => {{
        if crate::config::log_level() == "debug" {
            eprintln!($($arg)*);
        }
    }};
}

pub struct McpServer {
    db: Arc<Database>,
    skills: Arc<SkillRegistry>,
    agents: Arc<AgentRegistry>,
    orchestrator: Arc<Orchestrator>,
    antibrick: Arc<AntiBrickEngine>,
    watchdog: Arc<FilesystemWatchdog>,
    sessions: std::sync::RwLock<HashMap<String, SessionInfo>>,
    messages: std::sync::Mutex<Vec<AgentMessage>>,
    next_msg_id: std::sync::atomic::AtomicI64,
}

#[derive(Clone, Debug)]
struct SessionInfo {
    agent_type: String,
    project: String,
    last_seen: i64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct AgentMessage {
    pub id: i64,
    pub from: String,
    pub to: String,
    pub content: String,
    pub timestamp: i64,
}

impl McpServer {
    pub fn new(db: Arc<Database>, orchestrator: Arc<Orchestrator>) -> Self {
        Self {
            db,
            skills: Arc::new(SkillRegistry::new()),
            agents: Arc::new(AgentRegistry::new()),
            orchestrator,
            antibrick: Arc::new(AntiBrickEngine::new(AntiBrickConfig::default())),
            watchdog: Arc::new(FilesystemWatchdog::new(Default::default())),
            sessions: std::sync::RwLock::new(HashMap::new()),
            messages: std::sync::Mutex::new(Vec::new()),
            next_msg_id: std::sync::atomic::AtomicI64::new(1),
        }
    }

    pub fn init(&self) {
        self.db.init().expect("Failed to initialize database");
        self.skills.init().ok();
        self.agents.init().ok();
        info_log!("[Synapsis MCP] Server initialized");
    }

    pub fn run(&self) -> Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut reader = io::BufReader::new(stdin.lock());

        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(resp) = self.handle_message(trimmed) {
                writeln!(stdout, "{}", resp)?;
                stdout.flush()?;
            }
        }

        Ok(())
    }

    pub fn handle_message(&self, message: &str) -> Option<String> {
        let request: Value = match serde_json::from_str(message) {
            Ok(v) => v,
            Err(e) => {
                debug_log!("[Synapsis MCP] Parse error: {} | Message: '{}'", e, message);
                return Some(
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON"}})
                        .to_string(),
                );
            }
        };

        let is_tool_call = request["method"].as_str() == Some("tools/call");
        let tool_name = request["params"]["name"].as_str().unwrap_or("").to_string();
        let is_notif = request.get("id").is_none_or(|v| v.is_null());
        let result = self.handle_request(&request);

        match result {
            Ok(resp) => {
                let has_error = resp.get("error").is_some();
                let is_write_tool = matches!(
                    tool_name.as_str(),
                    "mem_save"
                        | "memory_add"
                        | "mem_delete"
                        | "db_backup"
                        | "db_prune"
                        | "task_create"
                        | "agent_register"
                        | "skill_register"
                        | "watchdog_snapshot"
                        | "antibrick_enable"
                );
                if is_tool_call && !has_error && !tool_name.is_empty() && is_write_tool {
                    self.auto_save_observation(&tool_name, &request["params"]["arguments"]);
                }
                if is_notif {
                    None
                } else {
                    serde_json::to_string(&resp).ok()
                }
            }
            Err(e) => {
                if is_notif {
                    None
                } else {
                    let err = json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "error": { "code": -32603, "message": e.to_string() }
                    });
                    serde_json::to_string(&err).ok()
                }
            }
        }
    }

    fn auto_save_observation(&self, tool_name: &str, args: &Value) {
        let content = format_args_snapshot(tool_name, args);
        let title = format!("tool:{}", tool_name);
        let mut obs = crate::domain::Observation::new(
            crate::domain::SessionId::new("mcp-auto"),
            crate::domain::ObservationType::Manual,
            title,
            content,
        );
        obs.project = Some("synapsis".to_string());
        obs.scope = crate::domain::Scope::Project;
        self.db.save_observation(&obs).ok();
    }

    fn handle_request(&self, request: &Value) -> Result<Value> {
        let method = request["method"].as_str().unwrap_or("");
        let id = &request["id"];

        match method {
            "initialize" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": { "listChanged": true },
                        "resources": { "listChanged": true },
                        "prompts": { "listChanged": true }
                    },
                    "serverInfo": { "name": "synapsis", "version": env!("CARGO_PKG_VERSION") }
                }
            })),
            "initialized" | "notifications/initialized" => Ok(json!({})),
            "tools/list" => self.list_tools(id),
            "tools/call" => self.call_tool(id, &request["params"]),
            "resources/list" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "resources": [
                    {"uri": "synapsis://memory", "name": "Synapsis Memory"},
                    {"uri": "synapsis://skills", "name": "Synapsis Skills"},
                    {"uri": "synapsis://agents", "name": "Synapsis Agents"}
                ]}
            })),
            "resources/read" => {
                let uri = request["params"]["uri"].as_str().unwrap_or("");
                let stats = self.db.stats().unwrap_or(json!({}));
                let default_zero = json!(0);
                let text = format!(
                    "Synapsis Memory Server v{}\nStats: {} observations",
                    env!("CARGO_PKG_VERSION"),
                    stats.get("observations").unwrap_or(&default_zero)
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "contents": [{"uri": uri, "text": text}] }
                }))
            }
            "prompts/list" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "prompts": [{"name": "memory_context"}] }
            })),
            "prompts/get" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "description": "Current Synapsis memory context",
                    "messages": [{
                        "role": "user",
                        "content": { "type": "text", "text": "Review Synapsis memory for relevant context." }
                    }]
                }
            })),
            _ => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {}", method) }
            })),
        }
    }

    fn list_tools(&self, id: &Value) -> Result<Value> {
        Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": [
                {
                    "name": "mem_save",
                    "description": "Save an observation to persistent memory.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "description": "Short searchable title" },
                            "content": { "type": "string", "description": "Structured content" },
                            "type": { "type": "string", "enum": ["decision","architecture","bugfix","pattern","config","discovery","learning","manual"], "default": "manual" },
                            "session_id": { "type": "string" },
                            "project": { "type": "string" },
                            "scope": { "type": "string", "enum": ["project","personal"], "default": "project" }
                        },
                        "required": ["title","content"]
                    }
                },
                {
                    "name": "mem_search",
                    "description": "Search persistent memory across all sessions.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string" },
                            "type": { "type": "string" },
                            "project": { "type": "string" },
                            "scope": { "type": "string", "enum": ["project","personal"] },
                            "limit": { "type": "integer", "default": 10 }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "mem_context",
                    "description": "Get recent context from current or previous sessions.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project": { "type": "string" },
                            "limit": { "type": "integer", "default": 5 }
                        }
                    }
                },
                {
                    "name": "mem_timeline",
                    "description": "Get chronological timeline of observations.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project": { "type": "string" },
                            "limit": { "type": "integer", "default": 20 },
                            "offset": { "type": "integer", "default": 0 }
                        }
                    }
                },
                {
                    "name": "mem_stats",
                    "description": "Get memory statistics.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "mem_delete",
                    "description": "Delete an observation by ID (soft-delete).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" }
                        },
                        "required": ["id"]
                    }
                },
                {
                    "name": "memory_search",
                    "description": "[alias] Search Synapsis persistent memory",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string" },
                            "limit": { "type": "integer", "default": 20 }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "memory_add",
                    "description": "[alias] Add observation to Synapsis",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "content": { "type": "string" },
                            "project": { "type": "string" }
                        },
                        "required": ["title","content"]
                    }
                },
                {
                    "name": "memory_timeline",
                    "description": "[alias] Get memory timeline",
                    "inputSchema": { "type": "object", "properties": { "limit": { "type": "integer", "default": 10 } } }
                },
                {
                    "name": "memory_stats",
                    "description": "[alias] Get memory statistics",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "ghost_audit",
                    "description": "Trigger proactive audit of a file or path",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "path": { "type": "string" } },
                        "required": ["path"]
                    }
                },
                {
                    "name": "pqc_encrypt",
                    "description": "Encrypt sensitive data using Post-Quantum Cryptography",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "plaintext": { "type": "string" } },
                        "required": ["plaintext"]
                    }
                },
                {
                    "name": "wasm_run",
                    "description": "Run a sandboxed WASM skill",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "wasm_hex": { "type": "string" },
                            "entry_func": { "type": "string", "default": "main" }
                        },
                        "required": ["wasm_hex"]
                    }
                },
                {
                    "name": "antibrick_scan",
                    "description": "Scan a command for potential brick threats",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "command": { "type": "string" },
                            "args": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["command","args"]
                    }
                },
                {
                    "name": "antibrick_stats",
                    "description": "Get anti-brick protection stats",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "antibrick_enable",
                    "description": "Enable or disable anti-brick protection",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "enable": { "type": "boolean" } },
                        "required": ["enable"]
                    }
                },
                {
                    "name": "watchdog_stats",
                    "description": "Get filesystem watchdog stats",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "watchdog_verify",
                    "description": "Verify integrity of monitored files",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "watchdog_snapshot",
                    "description": "Create integrity snapshot of a path",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "path": { "type": "string" } },
                        "required": ["path"]
                    }
                },
                {
                    "name": "watchdog_events",
                    "description": "Get recent watchdog events",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "limit": { "type": "integer", "default": 20 } }
                    }
                },
                {
                    "name": "watchdog_check_path",
                    "description": "Check if a path is protected by watchdog",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "path": { "type": "string" } },
                        "required": ["path"]
                    }
                },
                {
                    "name": "skill_register",
                    "description": "Register a new skill",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "description": { "type": "string" },
                            "category": { "type": "string", "default": "custom" }
                        },
                        "required": ["name"]
                    }
                },
                {
                    "name": "skill_list",
                    "description": "List all registered skills",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "agent_register",
                    "description": "Register a new agent",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "role": { "type": "string", "default": "general" },
                            "description": { "type": "string" }
                        },
                        "required": ["name"]
                    }
                },
                {
                    "name": "agent_list",
                    "description": "List all registered agents",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "task_create",
                    "description": "Create a new task",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "priority": { "type": "integer", "default": 1 }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "task_list",
                    "description": "List all tasks",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "mcp_call",
                    "description": "Call a tool on another MCP server.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "server_url": { "type": "string" },
                            "tool_name": { "type": "string" },
                            "arguments": { "type": "object", "default": {} },
                            "endpoint": { "type": "string", "default": "/message" }
                        },
                        "required": ["server_url", "tool_name"]
                    }
                },
                {
                    "name": "browser_navigate",
                    "description": "Fetch a web page as an HTTP client.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "url": { "type": "string" },
                            "method": { "type": "string", "enum": ["GET", "POST"], "default": "GET" },
                            "headers": { "type": "object", "default": {} },
                            "body": { "type": "string" },
                            "extract": { "type": "string", "enum": ["full", "text", "links", "meta"], "default": "full" }
                        },
                        "required": ["url"]
                    }
                },
                {
                    "name": "browser_snapshot",
                    "description": "Get a structured snapshot of a web page.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "url": { "type": "string" },
                            "max_text": { "type": "integer", "default": 2000 }
                        },
                        "required": ["url"]
                    }
                },
                {
                    "name": "db_backup",
                    "description": "Create a backup of the Synapsis database.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" }
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "db_integrity",
                    "description": "Run PRAGMA integrity_check on the database.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "db_prune",
                    "description": "Soft-delete observations older than N days.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "older_than_days": { "type": "integer", "default": 90 }
                        }
                    }
                },
                {
                    "name": "db_vacuum",
                    "description": "Reclaim unused space in the database (VACUUM).",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "mem_update",
                    "description": "Update an existing observation's title and content.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer", "description": "Observation ID" },
                            "title": { "type": "string", "description": "New title" },
                            "content": { "type": "string", "description": "New content" }
                        },
                        "required": ["id", "title", "content"]
                    }
                },
                {
                    "name": "mem_get_observation",
                    "description": "Get a single observation by ID with full content.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer", "description": "Observation ID" }
                        },
                        "required": ["id"]
                    }
                },
                {
                    "name": "mem_judge",
                    "description": "Record a judgment resolving a detected memory conflict.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "source_id": { "type": "integer", "description": "Source observation ID" },
                            "target_id": { "type": "integer", "description": "Target observation ID" },
                            "relation": { "type": "string", "enum": ["related","compatible","scoped","conflicts_with","supersedes","not_conflict"], "description": "Type of relation" },
                            "reason": { "type": "string", "description": "Explanation" },
                            "evidence": { "type": "string" },
                            "confidence": { "type": "number", "default": 1.0 },
                            "session_id": { "type": "string" },
                            "project": { "type": "string" }
                        },
                        "required": ["source_id", "target_id", "relation"]
                    }
                },
                {
                    "name": "mem_compare",
                    "description": "Directly compare two memories and record a semantic verdict.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "memory_id_a": { "type": "integer", "description": "First observation ID" },
                            "memory_id_b": { "type": "integer", "description": "Second observation ID" },
                            "relation": { "type": "string", "enum": ["related","compatible","scoped","conflicts_with","supersedes","not_conflict"] },
                            "confidence": { "type": "number", "default": 1.0 },
                            "reasoning": { "type": "string" },
                            "model": { "type": "string" }
                        },
                        "required": ["memory_id_a", "memory_id_b", "relation"]
                    }
                },
                {
                    "name": "mem_session_start",
                    "description": "Start a new memory session for tracking context across a work session.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": { "type": "string", "description": "Unique session identifier" },
                            "project": { "type": "string" }
                        }
                    }
                },
                {
                    "name": "mem_session_end",
                    "description": "End a memory session with an optional summary.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": { "type": "string" },
                            "summary": { "type": "string" }
                        },
                        "required": ["session_id"]
                    }
                },
                {
                    "name": "mem_session_summary",
                    "description": "Get a summary of a session: observation count, first/last activity.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": { "type": "string" }
                        }
                    }
                },
                {
                    "name": "mem_doctor",
                    "description": "Run diagnostics on the Synapsis memory database.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "mem_merge_projects",
                    "description": "Merge all observations from one project into another.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "source": { "type": "string", "description": "Source project to merge from" },
                            "target": { "type": "string", "description": "Target project to merge into" }
                        },
                        "required": ["source", "target"]
                    }
                },
                {
                    "name": "mem_current_project",
                    "description": "Get observation count and context for a project.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project": { "type": "string" }
                        }
                    }
                }
            ]}
        }))
    }

    fn call_tool(&self, id: &Value, params: &Value) -> Result<Value> {
        let name = params["name"].as_str().unwrap_or("");
        let args = &params["arguments"];

        match name {
            "mem_save" | "memory_add" => tools::handle_mem_save(&self.db, id, args),
            "mem_search" | "memory_search" => tools::handle_mem_search(&self.db, id, args),
            "mem_context" => tools::handle_mem_context(&self.db, id, args),
            "mem_timeline" | "memory_timeline" => tools::handle_mem_timeline(&self.db, id, args),
            "mem_stats" | "memory_stats" => tools::handle_mem_stats(&self.db, id),
            "mem_delete" => tools::handle_mem_delete(&self.db, id, args),
            "mem_update" => tools::handle_mem_update(&self.db, id, args),
            "mem_get_observation" => tools::handle_mem_get_observation(&self.db, id, args),
            "mem_judge" => tools::handle_mem_judge(&self.db, id, args),
            "mem_compare" => tools::handle_mem_compare(&self.db, id, args),
            "mem_session_start" => tools::handle_mem_session_start(&self.db, id, args),
            "mem_session_end" => tools::handle_mem_session_end(&self.db, id, args),
            "mem_session_summary" => tools::handle_mem_session_summary(&self.db, id, args),
            "mem_doctor" => tools::handle_mem_doctor(&self.db, id),
            "mem_merge_projects" => tools::handle_mem_merge_projects(&self.db, id, args),
            "mem_current_project" => tools::handle_mem_current_project(&self.db, id, args),
            "ghost_audit" => tools::handle_ghost_audit(&self.orchestrator, id, args),
            "pqc_encrypt" => tools::handle_pqc_encrypt(id, args),
            "wasm_run" => tools::handle_wasm_run(id, args),
            "antibrick_scan" => tools::handle_antibrick_scan(&self.antibrick, id, args),
            "antibrick_stats" => tools::handle_antibrick_stats(&self.antibrick, id),
            "antibrick_enable" => tools::handle_antibrick_enable(&self.antibrick, id, args),
            "watchdog_stats" => tools::handle_watchdog_stats(&self.watchdog, id),
            "watchdog_verify" => tools::handle_watchdog_verify(&self.watchdog, id),
            "watchdog_snapshot" => tools::handle_watchdog_snapshot(&self.watchdog, id, args),
            "watchdog_events" => tools::handle_watchdog_events(&self.watchdog, id, args),
            "watchdog_check_path" => tools::handle_watchdog_check_path(&self.watchdog, id, args),
            "db_backup" => tools::handle_db_backup(&self.db, id, args),
            "db_integrity" => tools::handle_db_integrity(&self.db, id),
            "db_prune" => tools::handle_db_prune(&self.db, id, args),
            "db_vacuum" => tools::handle_db_vacuum(&self.db, id),
            "skill_register" => tools::handle_skill_register(&self.skills, id, args),
            "skill_list" => tools::handle_skill_list(&self.skills, id),
            "agent_register" => tools::handle_agent_register(&self.agents, id, args),
            "agent_list" => tools::handle_agent_list(&self.agents, id),
            "task_create" => tools::handle_task_create(&self.orchestrator, id, args),
            "task_list" => tools::handle_task_list(&self.orchestrator, id),
            "mcp_call" => tools::handle_mcp_call(id, args),
            "browser_navigate" => tools::handle_browser_navigate(id, args),
            "browser_snapshot" => tools::handle_browser_snapshot(id, args),
            _ => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Unknown tool: {}", name) }
            })),
        }
    }

    pub fn register_session(&self, agent_type: &str, session_id: &str, project: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let mut sessions = self.sessions.write_safe();
        sessions.insert(
            session_id.to_string(),
            SessionInfo {
                agent_type: agent_type.to_string(),
                project: project.to_string(),
                last_seen: now,
            },
        );
    }

    pub fn send_message(&self, from: &str, to: &str, content: &str) -> i64 {
        let id = self
            .next_msg_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let msg = AgentMessage {
            id,
            from: from.to_string(),
            to: to.to_string(),
            content: content.to_string(),
            timestamp: now,
        };
        self.messages.lock_safe().push(msg);
        id
    }

    pub fn get_messages_for(&self, session_id: &str) -> Vec<AgentMessage> {
        let msgs = self.messages.lock_safe();
        msgs.iter()
            .filter(|m| m.to == session_id)
            .cloned()
            .collect()
    }

    pub fn get_active_agents(&self) -> Vec<serde_json::Value> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let sessions = self.sessions.read_safe();
        sessions
            .iter()
            .filter(|(_, info)| now - info.last_seen < 300)
            .map(|(id, info)| {
                json!({
                    "session_id": id,
                    "agent_type": info.agent_type,
                    "project": info.project,
                    "last_seen": info.last_seen,
                })
            })
            .collect()
    }
}
