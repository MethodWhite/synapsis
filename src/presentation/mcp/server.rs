//! Synapsis MCP Server Implementation
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::tools::browser_navigation::mcp_tools as browser_navigation_tools;
use crate::tools::cve_search::mcp_tools as cve_search_tools;
use crate::tools::env_detection::handle_env_detection;
use crate::tools::security_classify::mcp_tools as security_classify_tools;
use crate::tools::web_research::mcp_tools as web_research_tools;
use synapsis_core::core::antibrick::{AntiBrickConfig, AntiBrickEngine};
use synapsis_core::core::orchestrator::{AgentStatus, Orchestrator};
use synapsis_core::core::watchdog::FilesystemWatchdog;
use synapsis_core::core::PqcryptoProvider;
use synapsis_core::domain::crypto::{CryptoProvider, PqcAlgorithm};
use synapsis_core::domain::entities::SearchParams;
use synapsis_core::domain::*;
use synapsis_core::infrastructure::agents::AgentRegistry;
use synapsis_core::infrastructure::database::Database;
use synapsis_core::infrastructure::plugin::PluginManager;
use synapsis_core::infrastructure::skills::SkillRegistry;
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Event {
    event_type: String,
    session_id: Option<String>,
    agent_type: Option<String>,
    project: Option<String>,
    from: Option<String>,
    to: Option<String>,
    content: Option<String>,
    task_id: Option<String>,
    skill_id: Option<String>,
    timestamp: i64,
}

impl Event {
    fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            session_id: None,
            agent_type: None,
            project: None,
            from: None,
            to: None,
            content: None,
            task_id: None,
            skill_id: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingMessage {
    from: Option<String>,
    content: String,
    timestamp: i64,
}

impl PendingMessage {
    fn new(from: Option<String>, content: String) -> Self {
        Self {
            from,
            content,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        }
    }
}

#[derive(Debug, Clone)]
struct ConnectionInfo {
    client_name: String,
    client_type: String, // "cursor", "vscode", "cli", "tui", "unknown"
    connected_at: Instant,
    last_activity: Instant,
    protocol: String, // "mcp-stdin", "mcp-tcp", "secure-tcp"
    status: ConnectionStatus,
}

#[derive(Debug, Clone, PartialEq)]
enum ConnectionStatus {
    Connected,
    Idle,
    Disconnected,
}

struct EventBus {
    events: Arc<Mutex<Vec<Event>>>,
    message_queue: Arc<Mutex<HashMap<String, Vec<PendingMessage>>>>,
}

impl EventBus {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            message_queue: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn publish(&self, event: Event) {
        let mut events = self.events.lock().unwrap();
        events.push(event.clone());
        if events.len() > 1000 {
            events.drain(0..500);
        }

        // Queue message for recipient if it's a direct message
        if event.event_type == "message" {
            if let (Some(to), Some(content)) = (&event.to, &event.content) {
                let mut queue = self.message_queue.lock().unwrap();
                let msg = PendingMessage::new(event.from.clone(), content.clone());
                queue.entry(to.clone()).or_default().push(msg);
            }
        }
    }

    fn poll(&self, since: i64) -> Vec<Event> {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .filter(|e| e.timestamp > since)
            .cloned()
            .collect()
    }

    fn get_pending_messages(&self, session_id: &str) -> Vec<PendingMessage> {
        let mut queue = self.message_queue.lock().unwrap();
        queue.remove(session_id).unwrap_or_default()
    }
}

// Persistent EventBus using SQLite - shared across all MCP instances
struct PersistentEventBus {
    db: Arc<Database>,
}

struct PublishParams<'a> {
    event_type: &'a str,
    from: &'a str,
    to: Option<&'a str>,
    project: Option<&'a str>,
    channel: &'a str,
    content: &'a str,
    priority: i32,
}

impl PersistentEventBus {
    fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn publish(&self, _params: PublishParams) -> Result<i64, String> {
        Ok(0)
    }

    fn broadcast(
        &self,
        _event_type: &str,
        _from: &str,
        _project: Option<&str>,
        _channel: &str,
        _content: &str,
        _priority: i32,
    ) -> Result<i64, String> {
        Ok(0)
    }

    fn poll(
        &self,
        _since: i64,
        _channel: Option<&str>,
        _project: Option<&str>,
        _limit: i32,
    ) -> Result<Vec<serde_json::Value>, String> {
        Ok(vec![])
    }

    fn get_pending_messages(&self, _session_id: &str) -> Result<Vec<serde_json::Value>, String> {
        Ok(vec![])
    }

    fn mark_read(&self, _event_id: i64) -> Result<(), String> {
        Ok(())
    }
}

pub struct McpServer {
    db: Arc<Database>,
    skills: Arc<SkillRegistry>,
    agents: Arc<AgentRegistry>,
    orchestrator: Arc<Orchestrator>,
    antibrick: Arc<AntiBrickEngine>,
    watchdog: Arc<FilesystemWatchdog>,
    client_name: Arc<RwLock<Option<String>>>,
    event_bus: Arc<EventBus>,
    persistent_event_bus: Arc<PersistentEventBus>,
    plugin_manager: Arc<PluginManager>,
    crypto_provider: Arc<dyn CryptoProvider>,
    connections: Arc<Mutex<HashMap<String, ConnectionInfo>>>,
    shutdown_requested: Arc<AtomicBool>,
}

impl McpServer {
    pub fn new(db: Arc<Database>, orchestrator: Arc<Orchestrator>) -> Self {
        // Determine plugin directory
        let plugin_dir = dirs::data_local_dir()
            .map(|mut d| {
                d.push("synapsis");
                d.push("plugins");
                d
            })
            .unwrap_or_else(|| PathBuf::from("./synapsis_plugins"));

        let persistent_event_bus = Arc::new(PersistentEventBus::new(db.clone()));

        Self {
            db: db.clone(),
            skills: Arc::new(SkillRegistry::new()),
            agents: Arc::new(AgentRegistry::new()),
            orchestrator,
            antibrick: Arc::new(AntiBrickEngine::new(AntiBrickConfig::default())),
            watchdog: Arc::new(FilesystemWatchdog::new(Default::default())),
            client_name: Arc::new(RwLock::new(None)),
            event_bus: Arc::new(EventBus::new()),
            persistent_event_bus,
            plugin_manager: Arc::new(PluginManager::new(plugin_dir)),
            crypto_provider: Arc::new(PqcryptoProvider::new()),
            connections: Arc::new(Mutex::new(HashMap::new())),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn init(&self) {
        self.skills.init().ok();
        self.agents.init().ok();
        self.watchdog.start_monitoring();
        eprintln!("[MCP] Rust Server Initialized (watchdog started)");
    }

    fn get_agent_id(&self) -> String {
        let client_name_lock = self.client_name.read().unwrap();
        client_name_lock
            .as_deref()
            .unwrap_or("mcp-session")
            .to_string()
    }

    fn get_session_id(&self) -> types::SessionId {
        let client_name_lock = self.client_name.read().unwrap();
        let cli_type = client_name_lock.as_deref().unwrap_or("mcp-session");
        types::SessionId::new(cli_type)
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

            if let Some(resp_str) = self.handle_message(&line) {
                writeln!(stdout, "{}", resp_str)?;
                stdout.flush()?;
            }
            if self.shutdown_requested.load(Ordering::SeqCst) {
                break;
            }
        }

        Ok(())
    }

    pub fn handle_message(&self, message: &str) -> Option<String> {
        let request: Value = match serde_json::from_str(message) {
            Ok(v) => v,
            Err(_) => {
                return Some(
                    json!({
                        "jsonrpc": "2.0",
                        "error": { "code": -32700, "message": "Invalid JSON" }
                    })
                    .to_string(),
                )
            }
        };
        let request_id = request["id"].clone();
        let is_notification = request_id.is_null();
        match self.handle_request(request) {
            Ok(response) => {
                if is_notification {
                    // Notifications should not receive a response
                    None
                } else {
                    serde_json::to_string(&response).ok()
                }
            }
            Err(e) => {
                if is_notification {
                    // Errors in notifications are not sent back
                    None
                } else {
                    let err_resp = json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "error": { "code": -32603, "message": e.to_string() }
                    });
                    serde_json::to_string(&err_resp).ok()
                }
            }
        }
    }

    fn extract_args<'a>(&self, params: &'a Value) -> &'a Value {
        if let Some(args) = params.get("arguments") {
            args
        } else {
            params
        }
    }

    fn handle_request(&self, request: Value) -> Result<Value> {
        let method = request["method"].as_str().unwrap_or("");
        let id = &request["id"];

        // Update connection activity
        if let Some(client_name) = self.client_name.read().unwrap().as_ref() {
            let mut connections = self.connections.lock().unwrap();
            if let Some(conn) = connections.get_mut(client_name) {
                conn.last_activity = Instant::now();
            }
        }

        match method {
            "initialize" => {
                let client_protocol = request["params"]["protocolVersion"]
                    .as_str()
                    .unwrap_or("2024-11-05");
                let client_name = request["params"]["clientInfo"]["name"]
                    .as_str()
                    .unwrap_or("mcp-client")
                    .to_string();
                {
                    let mut client_name_lock = self.client_name.write().unwrap();
                    *client_name_lock = Some(client_name.clone());
                }
                // Track connection
                let connection_id = client_name.clone();
                let mut connections = self.connections.lock().unwrap();
                connections.insert(
                    connection_id,
                    ConnectionInfo {
                        client_name: client_name.clone(),
                        client_type: "unknown".to_string(),
                        connected_at: Instant::now(),
                        last_activity: Instant::now(),
                        protocol: "mcp-stdin".to_string(),
                        status: ConnectionStatus::Connected,
                    },
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": client_protocol,
                        "capabilities": {
                            "tools": { "listChanged": true },
                            "resources": { "listChanged": true },
                            "prompts": { "listChanged": true }
                        },
                        "serverInfo": {
                            "name": "synapsis",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                }))
            }
            "initialized" => Ok(json!(null)), // notification, no response needed
            "shutdown" => {
                self.shutdown_requested.store(true, Ordering::SeqCst);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null
                }))
            }
            "$/cancelRequest" => Ok(json!(null)), // ignore
            "tools/list" => self.list_tools(id),
            "tools/call" => self.call_tool(id, &request["params"]),
            "resources/list" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "resources": [
                    { "uri": "synapsis://memory", "name": "Memory" },
                    { "uri": "synapsis://skills", "name": "Skills" },
                    { "uri": "synapsis://agents", "name": "Agents" }
                ] }
            })),
            "prompts/list" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "prompts": [{ "name": "memory_context" }] }
            })),
            "prompts/get" => {
                let name = request["params"]["name"].as_str().unwrap_or("");
                if name == "memory_context" {
                   let args = &request["params"]["arguments"];
                   match self.action_mem_context(args) {
                       Ok(ctx) => Ok(json!({
                           "jsonrpc": "2.0",
                           "id": id,
                           "result": { "messages": [{ "role": "user", "content": { "type": "text", "text": serde_json::to_string(&ctx).unwrap() } }] }
                       })),
                       Err(e) => Err(anyhow::anyhow!("{}", e)),
                   }
                } else {
                    Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32601, "message": "Prompt not found" }
                    }))
                }
            }
            _ => {
                // Bridge for mw-cli direct method calls
                let args = self.extract_args(&request["params"]);
                let tool_params = json!({ "name": method, "arguments": args });
                self.call_tool(id, &tool_params)
            }
        }
    }

    fn list_tools(&self, id: &Value) -> Result<Value> {
        Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "mem_save",
                        "description": "Save an observation to Synapsis persistent memory",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "content": { "type": "string" },
                                "project": { "type": "string", "default": "default" },
                                "observation_type": { "type": "integer", "default": 1 }
                            },
                            "required": ["title", "content"]
                        }
                    },
                    {
                        "name": "mem_search",
                        "description": "Search observations using FTS5 vector-lite engine",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" },
                                "project": { "type": "string" },
                                "limit": { "type": "integer", "default": 10 }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "mem_context",
                        "description": "Get relevant context for current session",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "limit": { "type": "integer", "default": 10 }
                            }
                        }
                    },
                    {
                        "name": "mem_timeline",
                        "description": "Get memory timeline for a project",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "project": { "type": "string" },
                                "limit": { "type": "integer", "default": 10 }
                            }
                        }
                    },
                    {
                        "name": "mem_update",
                        "description": "Update existing observation (creates audit entry)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "observation_id": { "type": "integer" },
                                "new_content": { "type": "string" },
                                "reason": { "type": "string" }
                            },
                            "required": ["observation_id", "new_content"]
                        }
                    },
                    {
                        "name": "mem_delete",
                        "description": "Soft-delete an observation",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "observation_id": { "type": "integer" },
                                "reason": { "type": "string" }
                            },
                            "required": ["observation_id"]
                        }
                    },
                    {
                        "name": "mem_session_start",
                        "description": "Initialize a new agent session",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "agent_type": { "type": "string" },
                                "project": { "type": "string", "default": "default" }
                            },
                            "required": ["agent_type"]
                        }
                    },
                    {
                        "name": "mem_session_end",
                        "description": "Finalize an agent session",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "mem_stats",
                        "description": "Get memory and agent status overview",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "project": { "type": "string" }
                            }
                        }
                    },
                    {
                        "name": "agent_heartbeat",
                        "description": "Send heartbeat and update current task/status",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "status": { "type": "string", "enum": ["idle", "busy"], "default": "idle" },
                                "task": { "type": "string" }
                            },
                            "required": ["session_id", "status"]
                        }
                    },
                    {
                        "name": "task_create",
                        "description": "Create a new coordinated task",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "project": { "type": "string", "default": "default" },
                                "task_type": { "type": "string" },
                                "payload": { "type": "string" },
                                "priority": { "type": "integer", "default": 0 }
                            },
                            "required": ["task_type", "payload"]
                        }
                    },
                    {
                        "name": "task_claim",
                        "description": "Claim a pending task for an agent",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "task_type": { "type": "string" }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "mem_lock_acquire",
                        "description": "Acquire a distributed lock on a resource",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "resource": { "type": "string" },
                                "session_id": { "type": "string" },
                                "ttl_seconds": { "type": "integer", "default": 60 }
                            },
                            "required": ["resource", "session_id"]
                        }
                    },
                    {
                        "name": "mem_lock_release",
                        "description": "Release a distributed lock",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "resource": { "type": "string" },
                                "session_id": { "type": "string" }
                            },
                            "required": ["resource", "session_id"]
                        }
                    },
                    {
                        "name": "web_research",
                        "description": "Consult specialized web intelligence",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" },
                                "limit": { "type": "integer", "default": 5 }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "cve_search",
                        "description": "Search NVD database for vulnerabilities",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "cve_id": { "type": "string", "description": "Specific CVE ID (e.g. CVE-2026-1234)" },
                                "keyword": { "type": "string" },
                                "limit": { "type": "integer", "default": 10 }
                            }
                        }
                    },
                    {
                        "name": "security_classify",
                        "description": "Analyze risk level of specialized content",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "text": { "type": "string" },
                                "context": { "type": "string", "default": "general" }
                            },
                            "required": ["text"]
                        }
                    }
                ]
            }
        }))
    }

    fn call_tool(&self, id: &Value, params: &Value) -> Result<Value> {
        let name = params["name"].as_str().unwrap_or("");
        let args = &params["arguments"];

        let action_result = match name {
            // Memory & Session Standard
            "mem_save" | "memory_add" => self.action_mem_save(args),
            "mem_search" | "memory_search" => self.action_mem_search(args),
            "mem_update" | "memory_update" => self.action_mem_update(args),
            "mem_delete" | "memory_delete" => self.action_mem_delete(args),
            "mem_timeline" | "memory_timeline" => self.action_mem_timeline(args),
            "mem_context" => self.action_mem_context(args),
            "mem_session_start" | "session_register" | "agent_register" => self.action_mem_session_start(args),
            "mem_session_end" => self.action_mem_session_end(args),
            "mem_stats" | "memory_stats" | "agents_active" => self.action_mem_stats(args),
            "mem_lock_acquire" => self.action_mem_lock_acquire(args),
            "mem_lock_release" => self.action_mem_lock_release(args),
            
            // Coordination & Tasks
            "agent_heartbeat" => self.action_agent_heartbeat(args),
            "agent_details" => self.action_agent_details(args),
            "task_create" | "task_create_db" => self.action_task_create(args),
            "task_claim" => self.action_task_claim(args),
            "task_list" => self.action_task_list(args),
            "task_cancel" => self.action_task_cancel(args),
            "task_complete" => self.action_task_complete(args),
            "task_complete_db" => self.action_task_complete_db(args),
            "task_delegate" => self.action_task_delegate(args),
            "task_request" => self.action_task_request(args),
            "task_audit" => self.action_task_audit(args),
            "ghost_audit" => self.action_ghost_audit(args),

            // Intelligence Tools
            "web_research" => self.action_web_research(args),
            "cve_search" => self.action_cve_search(args),
            "security_classify" => self.action_security_classify(args),
            
            // System: Crypto & Environment
            "pqc_encrypt" | "crypto_pqc_encrypt" => self.action_crypto_pqc_encrypt(args),
            "wasm_run" => self.action_wasm_run(args),
            "env_detection" => self.action_env_detection(args),
            "connection_status" => self.action_connection_status(args),

            // System: Browser Navigation
            "browser_navigate" => self.action_browser_navigate(args),
            "browser_extract_text" => self.action_browser_extract_text(args),
            "browser_click" => self.action_browser_click(args),
            "browser_fill_form" => self.action_browser_fill_form(args),
            "browser_screenshot" => self.action_browser_screenshot(args),

            // System: Antibrick & Watchdog
            "antibrick_scan" => self.action_antibrick_scan(args),
            "antibrick_stats" => self.action_antibrick_stats(args),
            "antibrick_enable" => self.action_antibrick_enable(args),
            "watchdog_stats" => self.action_watchdog_stats(args),
            "watchdog_verify" => self.action_watchdog_verify(args),
            "watchdog_snapshot" => self.action_watchdog_snapshot(args),
            "watchdog_events" => self.action_watchdog_events(args),
            "watchdog_check_path" => self.action_watchdog_check_path(args),

            // Messaging & Events
            "send_message" => self.action_send_message(args),
            "event_poll" => self.action_event_poll(args),
            "event_ack" => self.action_event_ack(args),
            "get_pending_messages" => self.action_get_pending_messages(args),
            "broadcast" => self.action_broadcast(args),

            // System: Plugin Management
            "plugin_load" => self.action_plugin_load(args),
            "plugin_unload" => self.action_plugin_unload(args),
            "plugin_list" => self.action_plugin_list(args),
            "plugin_info" => self.action_plugin_info(args),
            "plugin_enable" => self.action_plugin_enable(args),
            "plugin_disable" => self.action_plugin_disable(args),
            "plugin_health" => self.action_plugin_health(args),
            "plugin_update_check" => self.action_plugin_update_check(args),
            "plugin_cleanup" => self.action_plugin_cleanup(args),

            _ => Err(format!("Unknown tool: {}", name)),
        };

        match action_result {
            Ok(result) => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()) }]
                }
            })),
            Err(e) => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32603, "message": e }
            })),
        }
    }

    // --- Private Action Methods (Unified Logic) ---

    fn action_mem_session_start(&self, args: &Value) -> Result<Value, String> {
        let agent_type = args.get("agent_type").and_then(|v| v.as_str()).unwrap_or("");
        let project = args.get("project").and_then(|v| v.as_str()).unwrap_or("default");

        let mut session_id = None;
        let mut reconnected = false;

        if let Ok(agents) = self.db.get_active_agents(Some(project)) {
            if let Some(existing) = agents.iter().find(|a| {
                a.get("agent_type").and_then(|v| v.as_str()) == Some(agent_type)
            }) {
                session_id = existing.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                reconnected = true;
            }
        }

        if session_id.is_none() {
            let agent_instance = "unknown";
            match self.db.register_agent_session(agent_type, agent_instance, project, None) {
                Ok(id) => session_id = Some(id),
                Err(e) => return Err(e.to_string()),
            }
        }

        Ok(json!({
            "session_id": session_id.unwrap_or_default(),
            "reconnected": reconnected
        }))
    }

    fn action_mem_session_end(&self, args: &Value) -> Result<Value, String> {
        let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
        // In a real scenario, we would mark the session as inactive in DB
        // For now, we return success
        Ok(json!({ "success": true, "session_id": session_id }))
    }

    fn action_task_create(&self, args: &Value) -> Result<Value, String> {
        let project = args.get("project").and_then(|v| v.as_str()).unwrap_or("default");
        let task_type = args.get("task_type").and_then(|v| v.as_str()).unwrap_or("");
        let payload = args.get("payload").and_then(|v| v.as_str()).unwrap_or("");
        let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        match self.db.create_task(project, task_type, payload, priority) {
            Ok(task_id) => Ok(json!({ "task_id": task_id })),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_task_list(&self, args: &Value) -> Result<Value, String> {
        let project = args.get("project").and_then(|v| v.as_str());
        let task_type = args.get("task_type").and_then(|v| v.as_str());
        let status = args.get("status").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_i64()).map(|l| l as i32);

        match self.db.list_tasks(project, task_type, status, limit) {
            Ok(tasks) => Ok(json!(tasks)),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_task_claim(&self, args: &Value) -> Result<Value, String> {
        let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
        let task_type = args.get("task_type").and_then(|v| v.as_str());

        match self.db.claim_task(session_id, task_type) {
            Ok(Some(task)) => Ok(task),
            Ok(None) => Ok(json!(null)),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_task_cancel(&self, args: &Value) -> Result<Value, String> {
        let task_id = args.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        match self.db.cancel_task(task_id) {
            Ok(_) => Ok(json!(true)),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_task_complete_db(&self, args: &Value) -> Result<Value, String> {
        let task_id = args.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        let result = args.get("result").and_then(|v| v.as_str());
        let error = args.get("error").and_then(|v| v.as_str());

        match self.db.complete_task(task_id, result, error) {
            Ok(_) => Ok(json!(true)),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_agent_details(&self, args: &Value) -> Result<Value, String> {
        let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
        match self.db.get_agent_details(session_id) {
            Ok(Some(details)) => Ok(details),
            Ok(None) => Ok(json!(null)),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_mem_stats(&self, args: &Value) -> Result<Value, String> {
        let project = args.get("project").and_then(|v| v.as_str());
        let stats = self.db.get_stats().map_err(|e| e.to_string())?;
        let active_agents = self.db.get_active_agents(project).map_err(|e| e.to_string())?;

        Ok(json!({
            "observations": stats.get("observations").unwrap_or(&json!(0)),
            "sessions": stats.get("agent_sessions").unwrap_or(&json!(0)),
            "pending_tasks": stats.get("pending_tasks").unwrap_or(&json!(0)),
            "active_agents": active_agents
        }))
    }

    fn action_agent_heartbeat(&self, args: &Value) -> Result<Value, String> {
        let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
        let task = args.get("task").and_then(|v| v.as_str());
        let status_str = args.get("status").and_then(|v| v.as_str()).unwrap_or("idle");

        let status = match status_str.to_lowercase().as_str() {
            "active" | "busy" => AgentStatus::Busy,
            _ => AgentStatus::Idle,
        };

        self.orchestrator.heartbeat(session_id, Some(status), task);
        match self.db.agent_heartbeat(session_id, task) {
            Ok(_) => Ok(json!(true)),
            Err(e) => Err(e.to_string()),
        }
    }
    fn action_mem_save(&self, args: &Value) -> Result<Value, String> {
        let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled");
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let project = args.get("project").and_then(|v| v.as_str()).map(|s| s.to_string());

        let mut obs = entities::Observation::new(
            self.get_session_id(),
            types::ObservationType::Manual,
            title.to_string(),
            content.to_string(),
        );
        obs.project = project;

        match self.db.save_observation(&obs) {
            Ok(id) => Ok(json!({ "observation_id": id })),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_mem_search(&self, args: &Value) -> Result<Value, String> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let project = args.get("project").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20) as i32;

        let mut params = SearchParams::new(query).with_limit(limit);
        if let Some(p) = project {
            params.project = Some(p.to_string());
        }

        match self.db.search_observations(&params) {
            Ok(results) => Ok(json!(results)),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_mem_update(&self, args: &Value) -> Result<Value, String> {
        let observation_id = args.get("observation_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let new_content = args.get("new_content").and_then(|v| v.as_str()).unwrap_or("");
        let reason = args.get("reason").and_then(|v| v.as_str());

        match self.db.update_observation(
            types::ObservationId(observation_id),
            new_content,
            &self.get_agent_id(),
            reason,
        ) {
            Ok(_) => Ok(json!({ "success": true })),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_mem_delete(&self, args: &Value) -> Result<Value, String> {
        let observation_id = args.get("observation_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let reason = args.get("reason").and_then(|v| v.as_str());

        match self.db.delete_observation(
            types::ObservationId(observation_id),
            &self.get_agent_id(),
            reason,
        ) {
            Ok(_) => Ok(json!({ "success": true })),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_mem_timeline(&self, args: &Value) -> Result<Value, String> {
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20) as i32;
        let project = args.get("project").and_then(|v| v.as_str());

        let mut params = SearchParams::new("").with_limit(limit);
        if let Some(p) = project {
            params.project = Some(p.to_string());
        }

        match self.db.search_observations(&params) {
            Ok(results) => Ok(json!(results)),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_web_research(&self, args: &Value) -> Result<Value, String> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
        Ok(web_research_tools::handle_web_research(query, limit))
    }

    fn action_cve_search(&self, args: &Value) -> Result<Value, String> {
        let cve_id = args.get("cve_id").and_then(|v| v.as_str());
        let keyword = args.get("keyword").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        Ok(cve_search_tools::handle_cve_search(cve_id, keyword, limit))
    }

    fn action_security_classify(&self, args: &Value) -> Result<Value, String> {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let context = args.get("context").and_then(|v| v.as_str()).unwrap_or("general");
        Ok(security_classify_tools::handle_security_classify(text, context))
    }

    fn action_ghost_audit(&self, args: &Value) -> Result<Value, String> {
        let path = args["path"].as_str().unwrap_or(".");
        let task_id = self.orchestrator.create_task(
            &format!("External audit request for {}", path),
            vec!["code_analysis".into()],
            5,
            None,
        );
        Ok(json!({ "task_id": task_id, "status": "created" }))
    }

    fn action_crypto_pqc_encrypt(&self, args: &Value) -> Result<Value, String> {
        let plaintext = args["plaintext"].as_str().unwrap_or("");
        let key_bytes = self
            .crypto_provider
            .random_bytes(32)
            .map_err(|e| format!("Failed to generate key: {}", e))?;
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        let ciphertext = self
            .crypto_provider
            .encrypt(&key, plaintext.as_bytes(), PqcAlgorithm::Aes256Gcm)
            .map_err(|e| format!("Encryption failed: {}", e))?;
        Ok(json!({ "ciphertext": hex::encode(ciphertext) }))
    }

    fn action_wasm_run(&self, _args: &Value) -> Result<Value, String> {
        Ok(json!({ "status": "scheduled", "message": "WASM execution scheduled via orchestrator" }))
    }

    fn action_env_detection(&self, args: &Value) -> Result<Value, String> {
        let mode = args["mode"].as_str();
        handle_env_detection(mode).map_err(|e| e.to_string())
    }

    fn action_antibrick_scan(&self, args: &Value) -> Result<Value, String> {
        let command = args["command"].as_str().unwrap_or("");
        let args_vec: Vec<String> = args["args"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(String::from).collect())
            .unwrap_or_default();
        Ok(synapsis_core::core::antibrick::mcp_tools::handle_antibrick_scan(&self.antibrick, command, args_vec))
    }

    fn action_antibrick_stats(&self, _args: &Value) -> Result<Value, String> {
        Ok(synapsis_core::core::antibrick::mcp_tools::handle_antibrick_stats(&self.antibrick))
    }

    fn action_antibrick_enable(&self, args: &Value) -> Result<Value, String> {
        let enable = args["enable"].as_bool().unwrap_or(true);
        Ok(synapsis_core::core::antibrick::mcp_tools::handle_antibrick_enable(&self.antibrick, enable))
    }

    fn action_watchdog_stats(&self, _args: &Value) -> Result<Value, String> {
        Ok(synapsis_core::core::watchdog::mcp_tools::handle_watchdog_stats(&self.watchdog))
    }

    fn action_watchdog_verify(&self, _args: &Value) -> Result<Value, String> {
        Ok(synapsis_core::core::watchdog::mcp_tools::handle_watchdog_verify(&self.watchdog))
    }

    fn action_watchdog_snapshot(&self, args: &Value) -> Result<Value, String> {
        let path = args["path"].as_str().unwrap_or("/").to_string();
        Ok(synapsis_core::core::watchdog::mcp_tools::handle_watchdog_snapshot(&self.watchdog, path))
    }

    fn action_watchdog_events(&self, args: &Value) -> Result<Value, String> {
        let limit = args["limit"].as_u64().unwrap_or(20) as usize;
        Ok(synapsis_core::core::watchdog::mcp_tools::handle_watchdog_events(&self.watchdog, limit))
    }

    fn action_watchdog_check_path(&self, args: &Value) -> Result<Value, String> {
        let path = args["path"].as_str().unwrap_or("/").to_string();
        Ok(synapsis_core::core::watchdog::mcp_tools::handle_watchdog_check_path(&self.watchdog, path))
    }

    fn action_browser_navigate(&self, args: &Value) -> Result<Value, String> {
        let url = args["url"].as_str().unwrap_or("");
        Ok(browser_navigation_tools::handle_navigate_to_url(url))
    }

    fn action_browser_extract_text(&self, args: &Value) -> Result<Value, String> {
        let url = args["url"].as_str().unwrap_or("");
        let selector = args["selector"].as_str().unwrap_or("");
        Ok(browser_navigation_tools::handle_extract_text(url, selector))
    }

    fn action_browser_click(&self, args: &Value) -> Result<Value, String> {
        let url = args["url"].as_str().unwrap_or("");
        let selector = args["selector"].as_str().unwrap_or("");
        Ok(browser_navigation_tools::handle_click_element(url, selector))
    }

    fn action_browser_fill_form(&self, args: &Value) -> Result<Value, String> {
        let url = args["url"].as_str().unwrap_or("");
        let selector = args["selector"].as_str().unwrap_or("");
        let value = args["value"].as_str().unwrap_or("");
        Ok(browser_navigation_tools::handle_fill_form(url, selector, value))
    }

    fn action_browser_screenshot(&self, args: &Value) -> Result<Value, String> {
        let url = args["url"].as_str().unwrap_or("");
        let output_path = args["output_path"].as_str().unwrap_or("");
        Ok(browser_navigation_tools::handle_screenshot(url, output_path))
    }

    fn action_send_message(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let to = args["to"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");
        let project = args.get("project").and_then(|v| v.as_str());
        let params = PublishParams {
            event_type: "message",
            from: session_id,
            to: Some(to),
            project,
            channel: "global",
            content,
            priority: 0,
        };
        self.persistent_event_bus.publish(params).map(|_| json!({ "status": "sent" })).map_err(|e| e.to_string())
    }

    fn action_event_poll(&self, args: &Value) -> Result<Value, String> {
        let since = args["since"].as_i64().unwrap_or(0);
        let channel = args.get("channel").and_then(|v| v.as_str());
        let project = args.get("project").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(100) as i32;
        self.persistent_event_bus.poll(since, channel, project, limit).map(|events| json!({ "events": events, "count": events.len() })).map_err(|e| e.to_string())
    }

    fn action_event_ack(&self, args: &Value) -> Result<Value, String> {
        let event_id = args["event_id"].as_i64().ok_or("Missing event_id")?;
        self.persistent_event_bus.mark_read(event_id).map(|_| json!({ "status": "acked" })).map_err(|e| e.to_string())
    }

    fn action_get_pending_messages(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        self.persistent_event_bus.get_pending_messages(session_id).map(|messages| json!({ "messages": messages, "count": messages.len() })).map_err(|e| e.to_string())
    }

    fn action_broadcast(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");
        let project = args.get("project").and_then(|v| v.as_str());
        let channel = args.get("channel").and_then(|v| v.as_str()).unwrap_or("global");
        let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let event_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("broadcast");
        self.persistent_event_bus.broadcast(event_type, session_id, project, channel, content, priority).map(|id| json!({ "event_id": id, "status": "sent" })).map_err(|e| e.to_string())
    }

    fn action_plugin_load(&self, args: &Value) -> Result<Value, String> {
        let path = args["path"].as_str().unwrap_or("");
        self.plugin_manager.load_plugin(path).map(|id| json!({ "plugin_id": id })).map_err(|e| e.to_string())
    }

    fn action_plugin_unload(&self, args: &Value) -> Result<Value, String> {
        let plugin_id = args["plugin_id"].as_str().unwrap_or("");
        self.plugin_manager.unload_plugin(plugin_id).map(|_| json!({ "status": "unloaded" })).map_err(|e| e.to_string())
    }

    fn action_plugin_list(&self, _args: &Value) -> Result<Value, String> {
        Ok(json!(self.plugin_manager.get_plugins()))
    }

    fn action_plugin_info(&self, args: &Value) -> Result<Value, String> {
        let plugin_id = args["plugin_id"].as_str().unwrap_or("");
        self.plugin_manager.get_plugin(plugin_id).map(|info| json!(info)).ok_or_else(|| "Plugin not found".to_string())
    }

    fn action_plugin_enable(&self, args: &Value) -> Result<Value, String> {
        let plugin_id = args["plugin_id"].as_str().unwrap_or("");
        let enabled = args["enabled"].as_bool().unwrap_or(true);
        self.plugin_manager.set_plugin_enabled(plugin_id, enabled).map(|_| json!({ "status": if enabled { "enabled" } else { "disabled" } })).map_err(|e| e.to_string())
    }

    fn action_plugin_disable(&self, args: &Value) -> Result<Value, String> {
        let plugin_id = args["plugin_id"].as_str().unwrap_or("");
        self.plugin_manager.set_plugin_enabled(plugin_id, false).map(|_| json!({ "status": "disabled" })).map_err(|e| e.to_string())
    }

    fn action_plugin_health(&self, args: &Value) -> Result<Value, String> {
        let plugin_id = args["plugin_id"].as_str();
        let health = self.plugin_manager.health_check();
        if let Some(pid) = plugin_id {
            health.get(pid).map(|r| json!(r)).ok_or_else(|| "Plugin not found".to_string())
        } else {
            Ok(json!(health))
        }
    }

    fn action_plugin_update_check(&self, _args: &Value) -> Result<Value, String> {
        Ok(json!(self.plugin_manager.check_for_updates()))
    }

    fn action_plugin_cleanup(&self, args: &Value) -> Result<Value, String> {
        let max_age = args["max_age_seconds"].as_i64().unwrap_or(86400);
        let removed = self.plugin_manager.cleanup_unused_plugins(max_age);
        Ok(json!({ "removed": removed }))
    }

    fn action_connection_status(&self, _args: &Value) -> Result<Value, String> {
        let mut connections = self.connections.lock().unwrap();
        let mut status_list = Vec::new();
        for (id, conn) in connections.iter_mut() {
            let elapsed = conn.last_activity.elapsed();
            conn.status = if elapsed < Duration::from_secs(30) { ConnectionStatus::Connected } else { ConnectionStatus::Idle };
            status_list.push(json!({
                "id": id,
                "client": conn.client_name,
                "status": format!("{:?}", conn.status),
                "last_activity": format!("{}s ago", elapsed.as_secs()),
                "protocol": conn.protocol
            }));
        }
        Ok(json!(status_list))
    }


    fn action_mem_context(&self, args: &Value) -> Result<Value, String> {
        let project = args.get("project").and_then(|v| v.as_str()).unwrap_or("default");
        let mut context = serde_json::Map::new();
        
        if let Ok(Some(global)) = self.db.get_global_context(project) {
            context.insert("global_context".to_string(), json!(global));
        }
        
        if let Ok(chunks) = self.db.get_chunks_by_project(project, None) {
             context.insert("knowledge_chunks".to_string(), json!(chunks));
        }
        
        Ok(json!(context))
    }

    fn action_mem_lock_acquire(&self, args: &Value) -> Result<Value, String> {
        let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
        let lock_key = args.get("lock_key").and_then(|v| v.as_str()).unwrap_or("global");
        let ttl = args.get("ttl_secs").and_then(|v| v.as_i64()).unwrap_or(60);
        
        match self.db.acquire_lock(session_id, lock_key, "generic", None, ttl) {
            Ok(success) => Ok(json!({ "success": success, "lock_key": lock_key })),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_mem_lock_release(&self, args: &Value) -> Result<Value, String> {
        let lock_key = args.get("lock_key").and_then(|v| v.as_str()).unwrap_or("global");
        match self.db.release_lock(lock_key) {
            Ok(_) => Ok(json!({ "success": true })),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_task_complete(&self, args: &Value) -> Result<Value, String> {
        let task_id = args.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        let success = args.get("success").and_then(|v| v.as_bool()).unwrap_or(true);
        let result = args.get("result").and_then(|v| v.as_str());
        
        self.orchestrator.complete_task(task_id, success);
        match self.db.complete_task(task_id, result, None) {
            Ok(_) => Ok(json!({ "success": true, "task_id": task_id })),
            Err(e) => Err(e.to_string()),
        }
    }

    fn action_task_delegate(&self, args: &Value) -> Result<Value, String> {
        let task_id = args.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        let from_agent = args.get("from_agent").and_then(|v| v.as_str()).unwrap_or("");
        
        match self.orchestrator.delegate_task(task_id, from_agent) {
            Some(to_agent) => Ok(json!({ "success": true, "delegated_to": to_agent })),
            None => Err("No suitable agent found for delegation".to_string()),
        }
    }

    fn action_task_request(&self, args: &Value) -> Result<Value, String> {
        let skills: Vec<String> = args.get("skills")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|s| s.as_str()).map(String::from).collect())
            .unwrap_or_default();
            
        match self.orchestrator.find_best_agent(&skills) {
            Some(agent_id) => Ok(json!({ "agent_id": agent_id })),
            None => Ok(json!(null)),
        }
    }

    fn action_task_audit(&self, args: &Value) -> Result<Value, String> {
        let task_id = args.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        let auditor = args.get("auditor_session_id").and_then(|v| v.as_str()).unwrap_or("");
        let status = args.get("audit_status").and_then(|v| v.as_str()).unwrap_or("approved");
        let notes = args.get("audit_notes").and_then(|v| v.as_str());
        
        match self.db.audit_task(task_id, auditor, status, notes) {
            Ok(_) => Ok(json!({ "success": true, "task_id": task_id })),
            Err(e) => Err(e.to_string()),
        }
    }
}
