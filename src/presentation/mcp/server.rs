use crate::core::lock_utils::*;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use crate::core::agent_registry_ext::AgentRegistryExt;
use crate::core::antibrick::{AntiBrickConfig, AntiBrickEngine};
use crate::core::auth::challenge::ChallengeResponse;
use crate::core::auth::classifier::AgentClassifier;
use crate::core::auth::tpm::TpmMfaProvider;
use crate::core::auto_integrate::AutoIntegrate;
use crate::core::chunk_query::ChunkQueryManager;
use crate::core::discovery::EnvironmentDiscovery;
use crate::core::orchestrator::Orchestrator;
use crate::core::recycle::RecycleBin;
use crate::core::resource_manager::ResourceManager;
use crate::core::session_manager::SessionManager;
use crate::core::sync::GitSyncEngine;
use crate::core::timeline_manager::TimelineManager;
use crate::core::tool_registry::ToolRegistryState;
use crate::core::vault::SecureVault;
use crate::core::watchdog::FilesystemWatchdog;
use crate::core::worker::{
    CodeWorker, FileWorker, GitWorker, SearchWorker, ShellWorker, WorkerOrchestrator,
};
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
    recycle: RecycleBin,
    agent_ext: AgentRegistryExt,
    session_mgr: SessionManager,
    timelines: TimelineManager,
    chunks: ChunkQueryManager,
    vault: SecureVault,
    workers: WorkerOrchestrator,
    git_sync: GitSyncEngine,
    auto_integrate: AutoIntegrate,
    tpm: TpmMfaProvider,
    resources: ResourceManager,
    classifier: Option<AgentClassifier>,
    #[allow(dead_code)]
    challenge: Option<ChallengeResponse>,
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
        let auth_enabled = std::env::var("SYNAPSIS_AUTH").is_ok();
        Self {
            db: db.clone(),
            recycle: RecycleBin::new(crate::config::data_dir()),
            agent_ext: AgentRegistryExt::new(db.clone()),
            session_mgr: SessionManager::new(db.clone()),
            timelines: TimelineManager::new(db.clone()),
            chunks: ChunkQueryManager::new(db.clone()),
            vault: SecureVault::new(crate::config::data_dir()),
            workers: {
                let mut wo = WorkerOrchestrator::new();
                wo.register_worker(Arc::new(ShellWorker::new().with_antibrick(Arc::new(
                    AntiBrickEngine::new(AntiBrickConfig::default()),
                ))));
                wo.register_worker(Arc::new(
                    FileWorker::new().with_allowed_dirs(vec![std::path::PathBuf::from(".")]),
                ));
                wo.register_worker(Arc::new(CodeWorker::new()));
                wo.register_worker(Arc::new(SearchWorker::new()));
                wo.register_worker(Arc::new(GitWorker::new()));
                wo
            },
            git_sync: GitSyncEngine::new(crate::core::sync::GitSyncConfig::default()),
            auto_integrate: {
                let discovery = Arc::new(EnvironmentDiscovery::new());
                let registry = ToolRegistryState::new();
                AutoIntegrate::new(discovery, registry)
            },
            tpm: TpmMfaProvider::new(),
            resources: ResourceManager::new(),
            classifier: auth_enabled.then(AgentClassifier::new),
            challenge: auth_enabled.then(ChallengeResponse::new),
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

        // Auth check: if SYNAPSIS_AUTH is set, require valid API key in initialize
        if let Some(ref _classifier) = self.classifier {
            let is_initialize = request["method"].as_str() == Some("initialize");
            if !is_initialize {
                let key = request["params"]["api_key"]
                    .as_str()
                    .or_else(|| request["params"]["token"].as_str())
                    .unwrap_or("");
                if key.is_empty() {
                    return Some(json!({"jsonrpc":"2.0","id":&request["id"],"error":{"code":-32001,"message":"Authentication required. Pass api_key in params."}}).to_string());
                }
            }
        }

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
                },
                {
                    "name": "mem_audit_log",
                    "description": "View the audit trail of observation changes (updates, deletes).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer", "default": 20 }
                        }
                    }
                },
                {
                    "name": "mem_recycle_save",
                    "description": "Save content to the recycle bin with a classification category.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string" },
                            "category": { "type": "string", "enum": ["critical","sensitive","important","standard","ephemeral"], "default": "standard" }
                        },
                        "required": ["content"]
                    }
                },
                {
                    "name": "mem_recycle_search",
                    "description": "Search recycled items by keyword and/or category.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "keyword": { "type": "string" },
                            "category": { "type": "string", "enum": ["critical","sensitive","important","standard","ephemeral"] },
                            "limit": { "type": "integer", "default": 20 },
                            "offset": { "type": "integer", "default": 0 }
                        }
                    }
                },
                {
                    "name": "mem_recycle_stats",
                    "description": "Get recycle bin statistics (total items, expired, bytes).",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "mem_recycle_delete",
                    "description": "Permanently delete an item from the recycle bin by ID.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" }
                        },
                        "required": ["id"]
                    }
                },
                {
                    "name": "agent_unregister",
                    "description": "Unregister an agent (mark as inactive).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent session ID" }
                        },
                        "required": ["agent_id"]
                    }
                },
                {
                    "name": "agent_list_by_project",
                    "description": "List all agents in a specific project.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project": { "type": "string" }
                        },
                        "required": ["project"]
                    }
                },
                {
                    "name": "chunk_query",
                    "description": "Query context chunks by chunk_id or by project.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "chunk_id": { "type": "string", "description": "Chunk ID to look up" },
                            "project": { "type": "string", "description": "Project key to list chunks" }
                        }
                    }
                },
                {
                    "name": "vault_store",
                    "description": "Store a secret in the secure vault.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "key": { "type": "string" },
                            "value": { "type": "string" }
                        },
                        "required": ["key", "value"]
                    }
                },
                {
                    "name": "vault_retrieve",
                    "description": "Retrieve a secret from the secure vault.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "key": { "type": "string" }
                        },
                        "required": ["key"]
                    }
                },
                {
                    "name": "worker_execute",
                    "description": "Execute a task on a worker agent (shell, file, code, search, git).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_type": { "type": "string", "description": "shell, file_read, file_write, code_analysis, code_refactor, search, git" },
                            "command": { "type": "string" },
                            "path": { "type": "string" },
                            "query": { "type": "string" },
                            "skill": { "type": "string" }
                        },
                        "required": ["task_type"]
                    }
                },
                {
                    "name": "worker_status",
                    "description": "List registered workers and external agents.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "sync_status",
                    "description": "Get Git sync engine status (commits, pending changes, conflicts).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "auto_discover",
                    "description": "Force a scan for available tools and auto-integrate them.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "auth_tpm_status",
                    "description": "Check TPM availability for hardware-backed security.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "auth_tpm_attest",
                    "description": "Generate a TPM attestation for a given nonce.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "nonce": { "type": "string" }
                        },
                        "required": ["nonce"]
                    }
                },
                {
                    "name": "auth_check_permission",
                    "description": "Check if a trust level grants a specific permission.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "permission": { "type": "string", "description": "pqc_encrypt, pqc_decrypt, manage_agents, configure_security, read_recycle, write_recycle, admin, create_task, execute_task" },
                            "trust_level": { "type": "string", "description": "all, trusted, basic, minimal, none" }
                        },
                        "required": ["permission"]
                    }
                },
                {
                    "name": "auth_classify_agent",
                    "description": "Classify an agent by type and get its trust level (requires SYNAPSIS_AUTH).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_type": { "type": "string" }
                        },
                        "required": ["agent_type"]
                    }
                },
                {
                    "name": "secure_write_file",
                    "description": "Write data to a file securely.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "data": { "type": "string" }
                        },
                        "required": ["path", "data"]
                    }
                },
                {
                    "name": "secure_read_file",
                    "description": "Read a file securely.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" }
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "secure_list_dir",
                    "description": "List directory contents securely.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" }
                        }
                    }
                },
                {
                    "name": "secure_random",
                    "description": "Generate a cryptographically secure random number (0 to max-1).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "max": { "type": "integer", "description": "Upper bound (exclusive), default 1000" }
                        }
                    }
                },
                {
                    "name": "vault_session_key",
                    "description": "Manage session keys in the secure vault (store, get, rotate, close).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": { "type": "string" },
                            "action": { "type": "string", "description": "store, get, rotate, close" },
                            "key_data": { "type": "string" }
                        },
                        "required": ["session_id", "action"]
                    }
                },
                {
                    "name": "vault_list_sessions",
                    "description": "List all active session keys in the vault.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "sync_memory",
                    "description": "Sync a memory observation to the Git sync engine.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string" },
                            "summary": { "type": "string" }
                        },
                        "required": ["summary"]
                    }
                },
                {
                    "name": "audit_log",
                    "description": "Get audit trail for a specific observation.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "observation_id": { "type": "integer" }
                        },
                        "required": ["observation_id"]
                    }
                },
                {
                    "name": "resource_stats",
                    "description": "Get system resource stats (CPU, memory, agents).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "resource_recommendations",
                    "description": "Get agent-specific resource recommendations.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string" }
                        },
                        "required": ["agent_id"]
                    }
                },
                {
                    "name": "orchestrator_tree",
                    "description": "Get sub-agent tree for a given agent.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string" }
                        },
                        "required": ["agent_id"]
                    }
                },
                {
                    "name": "orchestrator_idle",
                    "description": "List idle agents available for task assignment.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]}
        }))
    }

    fn call_tool(&self, id: &Value, params: &Value) -> Result<Value> {
        let name = params["name"].as_str().unwrap_or("");
        let args = &params["arguments"];

        match name {
            "mem_save" => tools::handle_mem_save(&self.db, id, args),
            "mem_search" => tools::handle_mem_search(&self.db, id, args),
            "mem_context" => tools::handle_mem_context(&self.db, id, args),
            "mem_timeline" => tools::handle_mem_timeline(&self.timelines, id, args),
            "mem_stats" => tools::handle_mem_stats(&self.db, id),
            "mem_delete" => tools::handle_mem_delete(&self.db, id, args),
            "mem_update" => tools::handle_mem_update(&self.db, id, args),
            "mem_get_observation" => tools::handle_mem_get_observation(&self.db, id, args),
            "mem_judge" => tools::handle_mem_judge(&self.db, id, args),
            "mem_compare" => tools::handle_mem_compare(&self.db, id, args),
            "mem_session_start" => tools::handle_mem_session_start(&self.session_mgr, id, args),
            "mem_session_end" => tools::handle_mem_session_end(&self.session_mgr, id, args),
            "mem_session_summary" => tools::handle_mem_session_summary(&self.session_mgr, id, args),
            "mem_doctor" => tools::handle_mem_doctor(&self.db, id),
            "mem_merge_projects" => tools::handle_mem_merge_projects(&self.db, id, args),
            "mem_current_project" => tools::handle_mem_current_project(&self.db, id, args),
            "mem_audit_log" => tools::handle_mem_audit_log(&self.db, id, args),
            "mem_recycle_save" => tools::handle_mem_recycle_save(&self.recycle, id, args),
            "mem_recycle_search" => tools::handle_mem_recycle_search(&self.recycle, id, args),
            "mem_recycle_stats" => tools::handle_mem_recycle_stats(&self.recycle, id),
            "mem_recycle_delete" => tools::handle_mem_recycle_delete(&self.recycle, id, args),
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
            "agent_unregister" => tools::handle_agent_unregister(&self.agent_ext, id, args),
            "agent_list_by_project" => {
                tools::handle_agent_list_by_project(&self.agent_ext, id, args)
            }
            "chunk_query" => tools::handle_chunk_query(&self.chunks, id, args),
            "vault_store" => tools::handle_vault_store(&self.vault, id, args),
            "vault_retrieve" => tools::handle_vault_retrieve(&self.vault, id, args),
            "worker_execute" => tools::handle_worker_execute(&self.workers, id, args),
            "worker_status" => tools::handle_worker_status(&self.workers, id),
            "sync_status" => tools::handle_sync_status(&self.git_sync, id),
            "auto_discover" => tools::handle_auto_discover(&self.auto_integrate, id),
            "auth_tpm_status" => tools::handle_auth_tpm_status(&self.tpm, id),
            "auth_tpm_attest" => tools::handle_auth_tpm_attest(&self.tpm, id, args),
            "auth_check_permission" => tools::handle_auth_check_permission(id, args),
            "secure_write_file" => tools::handle_secure_write_file(id, args),
            "secure_read_file" => tools::handle_secure_read_file(id, args),
            "secure_list_dir" => tools::handle_secure_list_dir(id, args),
            "secure_random" => tools::handle_secure_random(id, args),
            "vault_session_key" => tools::handle_vault_session_key(&self.vault, id, args),
            "vault_list_sessions" => tools::handle_vault_list_sessions(&self.vault, id),
            "sync_memory" => tools::handle_sync_memory(&self.git_sync, id, args),
            "audit_log" => tools::handle_audit_log(id, args),
            "resource_stats" => tools::handle_resource_stats(&self.resources, id),
            "resource_recommendations" => {
                tools::handle_resource_recommendations(&self.resources, id, args)
            }
            "orchestrator_tree" => tools::handle_orchestrator_tree(&self.orchestrator, id, args),
            "orchestrator_idle" => tools::handle_orchestrator_idle(&self.orchestrator, id),
            "auth_classify_agent" => match &self.classifier {
                Some(c) => tools::handle_auth_classify_agent(c, id, args),
                None => Ok(
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Auth not enabled (set SYNAPSIS_AUTH env var)"}}),
                ),
            },
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
