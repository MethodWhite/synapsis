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

    fn publish(&self, params: PublishParams) -> Result<i64, String> {
        self.db
            .publish_event(
                params.event_type,
                params.from,
                params.to,
                params.project,
                params.channel,
                params.content,
                params.priority,
            )
            .map_err(|e| e.to_string())
    }

    fn broadcast(
        &self,
        event_type: &str,
        from: &str,
        project: Option<&str>,
        channel: &str,
        content: &str,
        priority: i32,
    ) -> Result<i64, String> {
        self.db
            .broadcast_event(event_type, from, project, channel, content, priority)
            .map_err(|e| e.to_string())
    }

    fn poll(
        &self,
        since: i64,
        channel: Option<&str>,
        project: Option<&str>,
        limit: i32,
    ) -> Result<Vec<serde_json::Value>, String> {
        self.db
            .poll_events(since, channel, project, limit)
            .map_err(|e| e.to_string())
    }

    fn get_pending_messages(&self, session_id: &str) -> Result<Vec<serde_json::Value>, String> {
        self.db
            .get_pending_messages_for_session(session_id)
            .map_err(|e| e.to_string())
    }

    fn mark_read(&self, event_id: i64) -> Result<(), String> {
        self.db.mark_event_read(event_id).map_err(|e| e.to_string())
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
        let is_notification = request["id"].is_null();
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
                        "error": { "code": -32603, "message": e.to_string() }
                    });
                    serde_json::to_string(&err_resp).ok()
                }
            }
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
            "agents_active" => {
                let params = &request["params"];
                let project = params.get("project").and_then(|v| v.as_str());
                match self.db.get_active_agents(project) {
                    Ok(agents) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": { "agents": agents }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": e.to_string() }
                    })),
                }
            }
            "task_create" => {
                let params = &request["params"];
                let args = params.get("arguments").and_then(|v| v.as_object());
                let project = args
                    .and_then(|a| a.get("project"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let task_type = args
                    .and_then(|a| a.get("task_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let payload = args
                    .and_then(|a| a.get("payload"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let priority = args
                    .and_then(|a| a.get("priority"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                // Use the multi_agent module to create task
                match self.db.create_task(project, task_type, payload, priority) {
                    Ok(task_id) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": { "task_id": task_id }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": e.to_string() }
                    })),
                }
            }
            "session_register" => {
                let params = &request["params"];
                let args = params.get("arguments").and_then(|v| v.as_object());
                let agent_type = args
                    .and_then(|a| a.get("agent_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let project = args
                    .and_then(|a| a.get("project"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let agent_instance = "unknown"; // default instance
                match self
                    .db
                    .register_agent_session(agent_type, agent_instance, project, None)
                {
                    Ok(session_id) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": { "session_id": session_id }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": e.to_string() }
                    })),
                }
            }
            _ => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "Method not found" }
            })),
        }
    }

    fn list_tools(&self, id: &Value) -> Result<Value> {
        Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "memory_search",
                        "description": "Search Synapsis persistent memory",
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
                        "description": "Add observation to Synapsis",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "content": { "type": "string" },
                                "project": { "type": "string" }
                            },
                            "required": ["title", "content"]
                        }
                    },
                    {
                        "name": "memory_update",
                        "description": "Update observation with audit trail",
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
                        "name": "memory_delete",
                        "description": "Soft delete observation with audit trail",
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
                        "name": "memory_timeline",
                        "description": "Get memory timeline",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "limit": { "type": "integer", "default": 10 }
                            }
                        }
                    },
                    {
                        "name": "memory_stats",
                        "description": "Get memory statistics",
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "agent_register",
                        "description": "Register a new agent",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "role": { "type": "string" }
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
                        "name": "skill_register",
                        "description": "Register a new skill",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "description": { "type": "string" }
                            },
                            "required": ["name"]
                        }
                    },
                    {
                        "name": "skill_list",
                        "description": "List all skills",
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "task_create",
                        "description": "Create a new task",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" }
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
                        "name": "ghost_audit",
                        "description": "Trigger a proactive audit of a file",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "pqc_encrypt",
                        "description": "Encrypt sensitive data using MethodWhite Sovereign PQC",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "plaintext": { "type": "string" }
                            },
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
                                "command": { "type": "string", "description": "Command to analyze (e.g., 'dd', 'fastboot')" },
                                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments" }
                            },
                            "required": ["command", "args"]
                        }
                    },
                    {
                        "name": "antibrick_stats",
                        "description": "Get anti-brick protection statistics",
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "antibrick_enable",
                        "description": "Enable or disable anti-brick protection",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "enable": { "type": "boolean" }
                            },
                            "required": ["enable"]
                        }
                    },
                    {
                        "name": "watchdog_stats",
                        "description": "Get filesystem watchdog statistics",
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "watchdog_verify",
                        "description": "Verify integrity of monitored files",
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "watchdog_snapshot",
                        "description": "Create snapshot of a path for integrity monitoring",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Path to snapshot" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "watchdog_events",
                        "description": "Get recent watchdog events",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "limit": { "type": "integer", "default": 20 }
                            }
                        }
                    },
                    {
                        "name": "watchdog_check_path",
                        "description": "Check if a path is protected",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "web_research",
                        "description": "Research information from the web",
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
                        "description": "Search for CVEs (Common Vulnerabilities and Exposures)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "cve_id": { "type": "string" },
                                "keyword": { "type": "string" },
                                "limit": { "type": "integer", "default": 10 }
                            }
                        }
                    },
                    {
                        "name": "security_classify",
                        "description": "Classify security level of text",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "text": { "type": "string" },
                                "context": { "type": "string", "default": "general" }
                            },
                            "required": ["text"]
                        }
                    },
                    {
                        "name": "browser_navigate",
                        "description": "Navigate to a URL and return page HTML",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string" }
                            },
                            "required": ["url"]
                        }
                    },
                    {
                        "name": "browser_extract_text",
                        "description": "Extract text from elements matching CSS selector",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string" },
                                "selector": { "type": "string" }
                            },
                            "required": ["url", "selector"]
                        }
                    },
                    {
                        "name": "browser_click",
                        "description": "Click an element matching CSS selector",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string" },
                                "selector": { "type": "string" }
                            },
                            "required": ["url", "selector"]
                        }
                    },
                    {
                        "name": "browser_fill_form",
                        "description": "Fill a form field with a value",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string" },
                                "selector": { "type": "string" },
                                "value": { "type": "string" }
                            },
                            "required": ["url", "selector", "value"]
                        }
                    },
                    {
                        "name": "browser_screenshot",
                        "description": "Take screenshot of page",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string" },
                                "output_path": { "type": "string" }
                            },
                            "required": ["url", "output_path"]
                        }
                    },
                    {
                        "name": "env_detection",
                        "description": "Auto-detect installed CLIs, TUIs, and IDEs. Modes: 'all' (default), 'mcp_compatible', 'auto_config'",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "mode": {
                                    "type": "string",
                                    "enum": ["all", "mcp_compatible", "auto_config"],
                                    "default": "all"
                                }
                            }
                        }
                    },
                    {
                        "name": "agent_heartbeat",
                        "description": "Update agent heartbeat and status",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "status": { "type": "string", "enum": ["idle", "busy"] },
                                "task": { "type": "string" }
                            },
                            "required": ["session_id", "status"]
                        }
                    },
                    {
                        "name": "agent_details",
                        "description": "Get details about a specific agent",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "send_message",
                        "description": "Send a message to another agent",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "to": { "type": "string" },
                                "content": { "type": "string" }
                            },
                            "required": ["session_id", "to", "content"]
                        }
                    },
                    {
                        "name": "agents_active",
                        "description": "List active agents in a project",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "project": { "type": "string" }
                            }
                        }
                    },
                    {
                        "name": "task_delegate",
                        "description": "Delegate a task to another agent",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": { "type": "string" },
                                "to_agent": { "type": "string" }
                            },
                            "required": ["task_id", "to_agent"]
                        }
                    },
                    {
                        "name": "task_claim",
                        "description": "Claim a pending task",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "task_request",
                        "description": "Request a task assignment",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "skills": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "task_complete",
                        "description": "Mark a task as completed",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": { "type": "string" },
                                "success": { "type": "boolean", "default": true },
                                "result": { "type": "string" }
                            },
                            "required": ["task_id"]
                        }
                    },
                    {
                        "name": "task_audit",
                        "description": "Audit a task with status and notes",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": { "type": "string" },
                                "auditor_session_id": { "type": "string" },
                                "audit_status": { "type": "string", "default": "approved" },
                                "audit_notes": { "type": "string" }
                            },
                            "required": ["task_id", "auditor_session_id"]
                        }
                    },
                    {
                        "name": "event_poll",
                        "description": "Poll for new events since timestamp (shared across all MCP instances)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "since": { "type": "integer", "description": "Poll events created after this Unix timestamp" },
                                "channel": { "type": "string", "description": "Filter by channel (default: 'global')" },
                                "project": { "type": "string", "description": "Filter by project" },
                                "limit": { "type": "integer", "default": 100, "description": "Max events to return" }
                            },
                            "required": ["since"]
                        }
                    },
                    {
                        "name": "get_pending_messages",
                        "description": "Get pending direct messages for session",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "event_ack",
                        "description": "Mark an event as read",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "event_id": { "type": "integer" }
                            },
                            "required": ["event_id"]
                        }
                    },
                    {
                        "name": "broadcast",
                        "description": "Broadcast message to all sessions in a channel/project (inter-agent communication)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string", "description": "Your session ID" },
                                "content": { "type": "string", "description": "Message content (JSON recommended for structured data)" },
                                "project": { "type": "string", "description": "Target project (optional)" },
                                "channel": { "type": "string", "default": "global", "description": "Channel name (all sessions in this channel receive the message)" },
                                "priority": { "type": "integer", "default": 0, "description": "Priority: 0=normal, 1=high, 2=critical" },
                                "type": { "type": "string", "default": "broadcast", "description": "Event type: broadcast, status_update, coordination, etc." }
                            },
                            "required": ["session_id", "content"]
                        }
                    },
                    {
                        "name": "plugin_load",
                        "description": "Load a plugin from path",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "plugin_unload",
                        "description": "Unload a plugin",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "plugin_id": { "type": "string" }
                            },
                            "required": ["plugin_id"]
                        }
                    },
                    {
                        "name": "plugin_list",
                        "description": "List all loaded plugins",
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "plugin_info",
                        "description": "Get plugin information",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "plugin_id": { "type": "string" }
                            },
                            "required": ["plugin_id"]
                        }
                    },
                    {
                        "name": "plugin_enable",
                        "description": "Enable a plugin",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "plugin_id": { "type": "string" },
                                "enabled": { "type": "boolean", "default": true }
                            },
                            "required": ["plugin_id"]
                        }
                    },
                    {
                        "name": "plugin_disable",
                        "description": "Disable a plugin",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "plugin_id": { "type": "string" }
                            },
                            "required": ["plugin_id"]
                        }
                    },
                    {
                        "name": "plugin_health",
                        "description": "Check plugin health",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "plugin_id": { "type": "string" }
                            }
                        }
                    },
                    {
                        "name": "plugin_update_check",
                        "description": "Check for plugin updates",
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "plugin_cleanup",
                        "description": "Clean up unused plugins",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "max_age_seconds": { "type": "integer", "default": 86400 }
                            }
                        }
                    },
                    {
                        "name": "connection_status",
                        "description": "Check connection status with green/red indicators",
                        "inputSchema": { "type": "object", "properties": {} }
                    }
                ]
            }
        }))
    }

    fn call_tool(&self, id: &Value, params: &Value) -> Result<Value> {
        let name = params["name"].as_str().unwrap_or("");
        let args = &params["arguments"];

        match name {
            "memory_search" => {
                let query = args["query"].as_str().unwrap_or("");
                let params = SearchParams::new(query);
                let results = self.db.search_observations(&params)?;

                let formatted_results: Vec<String> = results
                    .iter()
                    .map(|obs| {
                        format!(
                            "- [{}] {}",
                            obs.observation.observation_type, obs.observation.title
                        )
                    })
                    .collect();

                let response = if formatted_results.is_empty() {
                    format!("No results found for query: '{}'.", query)
                } else {
                    format!(
                        "Found {} results for query: '{}':\n{}",
                        results.len(),
                        query,
                        formatted_results.join("\n")
                    )
                };

                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": response }]
                    }
                }))
            }
            "memory_add" => {
                let title = args["title"].as_str().unwrap_or("Untitled");
                let content = args["content"].as_str().unwrap_or("");
                let project = args["project"].as_str().map(|s| s.to_string());

                let mut obs = entities::Observation::new(
                    self.get_session_id(),
                    types::ObservationType::Manual,
                    title.to_string(),
                    content.to_string(),
                );
                obs.project = project;

                let obs_id = self.db.save_observation(&obs)?;
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Added observation {}", obs_id) }]
                    }
                }))
            }
            "memory_update" => {
                let observation_id = args["observation_id"].as_i64().unwrap_or(0);
                let new_content = args["new_content"].as_str().unwrap_or("");
                let reason = args["reason"].as_str();
                // Use client-provided agent_id
                self.db.update_observation(
                    types::ObservationId(observation_id),
                    new_content,
                    &self.get_agent_id(),
                    reason,
                )?;
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Updated observation {}", observation_id) }]
                    }
                }))
            }
            "memory_delete" => {
                let observation_id = args["observation_id"].as_i64().unwrap_or(0);
                let reason = args["reason"].as_str();
                self.db.delete_observation(
                    types::ObservationId(observation_id),
                    &self.get_agent_id(),
                    reason,
                )?;
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Soft-deleted observation {}", observation_id) }]
                    }
                }))
            }
            "memory_timeline" => {
                let limit = args["limit"].as_i64().unwrap_or(20) as i32;
                let params = SearchParams::new("").with_limit(limit);
                let observations = self.db.search_observations(&params)?;

                let formatted: Vec<String> = observations
                    .iter()
                    .map(|obs| {
                        format!(
                            "- {} (Session: {})",
                            obs.observation.title, obs.observation.session_id
                        )
                    })
                    .collect();

                let response = if formatted.is_empty() {
                    "Timeline: No observations found.".to_string()
                } else {
                    format!(
                        "Timeline ({} most recent):\n{}",
                        observations.len(),
                        formatted.join("\n")
                    )
                };

                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "content": [{ "type": "text", "text": response }] }
                }))
            }
            "memory_stats" => {
                let stats = self.db.get_stats()?;
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": format!(
                                "Observations: {}\nSessions: {}\nActive Agents: {}\nPending Tasks: {}",
                                stats.get("observations").unwrap_or(&serde_json::Value::from(0)),
                                stats.get("agent_sessions").unwrap_or(&serde_json::Value::from(0)),
                                stats.get("active_agents").unwrap_or(&serde_json::Value::from(0)),
                                stats.get("pending_tasks").unwrap_or(&serde_json::Value::from(0))
                            )
                        }]
                    }
                }))
            }
            "agent_register" | "agent_list" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "content": [{ "type": "text", "text": "Registered agent" }] }
            })),
            "skill_register" | "skill_list" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "content": [{ "type": "text", "text": "Registered skill" }] }
            })),
            "task_create" | "task_list" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "content": [{ "type": "text", "text": "Task created" }] }
            })),
            "ghost_audit" => {
                let path = args["path"].as_str().unwrap_or(".");
                let task_id = self.orchestrator.create_task(
                    &format!("External audit request for {}", path),
                    vec!["code_analysis".into()],
                    5,
                    None,
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Audit task {} created", task_id) }]
                    }
                }))
            }
            "pqc_encrypt" => {
                let plaintext = args["plaintext"].as_str().unwrap_or("");
                let key_bytes = self
                    .crypto_provider
                    .random_bytes(32)
                    .map_err(|e| anyhow::anyhow!("Failed to generate key: {}", e))?;
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_bytes);
                let ciphertext = self
                    .crypto_provider
                    .encrypt(&key, plaintext.as_bytes(), PqcAlgorithm::Aes256Gcm)
                    .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": hex::encode(ciphertext) }]
                    }
                }))
            }
            "wasm_run" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": "WASM execution scheduled via orchestrator." }]
                }
            })),
            "antibrick_scan" => {
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

                let result = synapsis_core::core::antibrick::mcp_tools::handle_antibrick_scan(
                    &self.antibrick,
                    command,
                    args_vec,
                );

                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "antibrick_stats" => {
                let stats = synapsis_core::core::antibrick::mcp_tools::handle_antibrick_stats(
                    &self.antibrick,
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": stats.to_string() }]
                    }
                }))
            }
            "antibrick_enable" => {
                let enable = args["enable"].as_bool().unwrap_or(true);
                let result = synapsis_core::core::antibrick::mcp_tools::handle_antibrick_enable(
                    &self.antibrick,
                    enable,
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "watchdog_stats" => {
                let stats =
                    synapsis_core::core::watchdog::mcp_tools::handle_watchdog_stats(&self.watchdog);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": stats.to_string() }]
                    }
                }))
            }
            "watchdog_verify" => {
                let result = synapsis_core::core::watchdog::mcp_tools::handle_watchdog_verify(
                    &self.watchdog,
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "watchdog_snapshot" => {
                let path = args["path"].as_str().unwrap_or("/").to_string();
                let result = synapsis_core::core::watchdog::mcp_tools::handle_watchdog_snapshot(
                    &self.watchdog,
                    path,
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "watchdog_events" => {
                let limit = args["limit"].as_u64().unwrap_or(20) as usize;
                let result = synapsis_core::core::watchdog::mcp_tools::handle_watchdog_events(
                    &self.watchdog,
                    limit,
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "watchdog_check_path" => {
                let path = args["path"].as_str().unwrap_or("/").to_string();
                let result = synapsis_core::core::watchdog::mcp_tools::handle_watchdog_check_path(
                    &self.watchdog,
                    path,
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "web_research" => {
                let query = args["query"].as_str().unwrap_or("");
                let limit = args["limit"].as_u64().unwrap_or(5) as usize;
                let result = web_research_tools::handle_web_research(query, limit);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "cve_search" => {
                let cve_id = args["cve_id"].as_str();
                let keyword = args["keyword"].as_str();
                let limit = args["limit"].as_u64().unwrap_or(10) as usize;
                let result = cve_search_tools::handle_cve_search(cve_id, keyword, limit);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "security_classify" => {
                let text = args["text"].as_str().unwrap_or("");
                let context = args["context"].as_str().unwrap_or("general");
                let result = security_classify_tools::handle_security_classify(text, context);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "browser_navigate" => {
                let url = args["url"].as_str().unwrap_or("");
                let result = browser_navigation_tools::handle_navigate_to_url(url);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "browser_extract_text" => {
                let url = args["url"].as_str().unwrap_or("");
                let selector = args["selector"].as_str().unwrap_or("");
                let result = browser_navigation_tools::handle_extract_text(url, selector);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "browser_click" => {
                let url = args["url"].as_str().unwrap_or("");
                let selector = args["selector"].as_str().unwrap_or("");
                let result = browser_navigation_tools::handle_click_element(url, selector);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "browser_fill_form" => {
                let url = args["url"].as_str().unwrap_or("");
                let selector = args["selector"].as_str().unwrap_or("");
                let value = args["value"].as_str().unwrap_or("");
                let result = browser_navigation_tools::handle_fill_form(url, selector, value);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "browser_screenshot" => {
                let url = args["url"].as_str().unwrap_or("");
                let output_path = args["output_path"].as_str().unwrap_or("");
                let result = browser_navigation_tools::handle_screenshot(url, output_path);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "env_detection" => {
                let mode = args["mode"].as_str();
                let result = match handle_env_detection(mode) {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32603, "message": e.to_string() }
                        }));
                    }
                };
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "agent_heartbeat" => {
                let session_id = args["session_id"].as_str().unwrap_or("");
                let status_str = args["status"].as_str().unwrap_or("idle");
                let task = args["task"].as_str();
                let status = match status_str {
                    "idle" => AgentStatus::Idle,
                    "busy" => AgentStatus::Busy,
                    _ => AgentStatus::Idle,
                };
                self.orchestrator.heartbeat(session_id, Some(status), task);
                // Update connection activity
                if let Some(client_name) = self.client_name.read().unwrap().as_ref() {
                    let mut connections = self.connections.lock().unwrap();
                    if let Some(conn) = connections.get_mut(client_name) {
                        conn.last_activity = Instant::now();
                    }
                }
                if let Err(e) = self.db.agent_heartbeat(session_id, task) {
                    return Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("{:?}", e) }
                    }));
                }
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": "Heartbeat updated" }]
                    }
                }))
            }
            "agent_details" => {
                let session_id = args["session_id"].as_str().unwrap_or("");
                match self.db.get_agent_details(session_id) {
                    Ok(details) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("{:?}", details) }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("{:?}", e) }
                    })),
                }
            }
            "send_message" => {
                let session_id = args["session_id"].as_str().unwrap_or("");
                let to = args["to"].as_str().unwrap_or("");
                let content = args["content"].as_str().unwrap_or("");
                let project = args.get("project").and_then(|v| v.as_str());

                // Publish to persistent event bus (shared across all MCP instances)
                let params = PublishParams {
                    event_type: "message",
                    from: session_id,
                    to: Some(to),
                    project,
                    channel: "global",
                    content,
                    priority: 0,
                };
                let event_id = self.persistent_event_bus.publish(params);

                match event_id {
                    Ok(_) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": "Message sent" }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": e }
                    })),
                }
            }
            "agents_active" => {
                let project = args["project"].as_str();
                match self.db.get_active_agents(project) {
                    Ok(agents) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("{:?}", agents) }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("{:?}", e) }
                    })),
                }
            }
            "task_delegate" => {
                let task_id = args["task_id"].as_str().unwrap_or("");
                let to_agent = args["to_agent"].as_str().unwrap_or("");
                if self.orchestrator.assign_task(task_id, to_agent) {
                    Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Task {} delegated to {}", task_id, to_agent) }]
                        }
                    }))
                } else {
                    Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": "Failed to delegate task" }
                    }))
                }
            }
            "task_claim" => {
                let session_id = args["session_id"].as_str().unwrap_or("");
                match self.db.claim_task(session_id, None) {
                    Ok(task) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Claimed task {:?}", task) }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("{:?}", e) }
                    })),
                }
            }
            "task_request" => {
                let session_id = args["session_id"].as_str().unwrap_or("");
                let skills: Vec<String> = args["skills"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                match self.orchestrator.find_best_agent(&skills) {
                    Some(agent_id) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Best agent: {}", agent_id) }]
                        }
                    })),
                    None => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": "No suitable agent found" }]
                        }
                    })),
                }
            }
            "task_complete" => {
                let task_id = args["task_id"].as_str().unwrap_or("");
                let success = args["success"].as_bool().unwrap_or(true);
                let result = args["result"].as_str();
                self.orchestrator.complete_task(task_id, success);
                if let Some(result_text) = result {
                    let _ = self.db.complete_task(task_id, Some(result_text), None);
                }
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Task {} completed", task_id) }]
                    }
                }))
            }
            "task_audit" => {
                let task_id = args["task_id"].as_str().unwrap_or("");
                let auditor_session_id = args["auditor_session_id"].as_str().unwrap_or("");
                let audit_status = args["audit_status"].as_str().unwrap_or("approved");
                let audit_notes = args["audit_notes"].as_str();
                match self
                    .db
                    .audit_task(task_id, auditor_session_id, audit_status, audit_notes)
                {
                    Ok(_) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Task {} audited", task_id) }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("{:?}", e) }
                    })),
                }
            }
            "event_poll" => {
                let since = args["since"].as_i64().unwrap_or(0);
                let channel = args.get("channel").and_then(|v| v.as_str());
                let project = args.get("project").and_then(|v| v.as_str());
                let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(100) as i32;

                let events = self
                    .persistent_event_bus
                    .poll(since, channel, project, limit);

                match events {
                    Ok(events) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "events": events,
                            "count": events.len()
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": e }
                    })),
                }
            }
            "get_pending_messages" => {
                let session_id = args["session_id"].as_str().unwrap_or("");
                let messages = self.persistent_event_bus.get_pending_messages(session_id);

                match messages {
                    Ok(messages) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "messages": messages,
                            "count": messages.len()
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": e }
                    })),
                }
            }
            "event_ack" => {
                let event_id = args["event_id"].as_i64();
                match event_id {
                    Some(id) => match self.persistent_event_bus.mark_read(id) {
                        Ok(_) => Ok(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": "Event marked as read" }]
                            }
                        })),
                        Err(e) => Ok(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32603, "message": e }
                        })),
                    },
                    None => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32602, "message": "Missing event_id parameter" }
                    })),
                }
            }
            "broadcast" => {
                let session_id = args["session_id"].as_str().unwrap_or("");
                let content = args["content"].as_str().unwrap_or("");
                let project = args.get("project").and_then(|v| v.as_str());
                let channel = args
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .unwrap_or("global");
                let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let event_type = args
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("broadcast");

                // Broadcast to all sessions in the channel/project
                let event_id = self
                    .persistent_event_bus
                    .broadcast(event_type, session_id, project, channel, content, priority);

                match event_id {
                    Ok(id) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Broadcast sent to channel '{}' (event_id: {})", channel, id) }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": e }
                    })),
                }
            }
            "plugin_load" => {
                let path = args["path"].as_str().unwrap_or("");
                match self.plugin_manager.load_plugin(path) {
                    Ok(plugin_id) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Plugin loaded with ID: {}", plugin_id) }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("Failed to load plugin: {}", e) }
                    })),
                }
            }
            "plugin_unload" => {
                let plugin_id = args["plugin_id"].as_str().unwrap_or("");
                match self.plugin_manager.unload_plugin(plugin_id) {
                    Ok(_) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Plugin {} unloaded", plugin_id) }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("Failed to unload plugin: {}", e) }
                    })),
                }
            }
            "plugin_list" => {
                let plugins = self.plugin_manager.get_plugins();
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("{:?}", plugins) }]
                    }
                }))
            }
            "plugin_info" => {
                let plugin_id = args["plugin_id"].as_str().unwrap_or("");
                match self.plugin_manager.get_plugin(plugin_id) {
                    Some(info) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("{:?}", info) }]
                        }
                    })),
                    None => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("Plugin not found: {}", plugin_id) }
                    })),
                }
            }
            "plugin_enable" => {
                let plugin_id = args["plugin_id"].as_str().unwrap_or("");
                let enabled = args["enabled"].as_bool().unwrap_or(true);
                match self.plugin_manager.set_plugin_enabled(plugin_id, enabled) {
                    Ok(_) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Plugin {} {}", plugin_id, if enabled { "enabled" } else { "disabled" }) }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("Failed to set plugin state: {}", e) }
                    })),
                }
            }
            "plugin_disable" => {
                let plugin_id = args["plugin_id"].as_str().unwrap_or("");
                match self.plugin_manager.set_plugin_enabled(plugin_id, false) {
                    Ok(_) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("Plugin {} disabled", plugin_id) }]
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32603, "message": format!("Failed to disable plugin: {}", e) }
                    })),
                }
            }
            "plugin_health" => {
                let plugin_id = args["plugin_id"].as_str();
                let health_results = self.plugin_manager.health_check();
                if let Some(pid) = plugin_id {
                    match health_results.get(pid) {
                        Some(result) => Ok(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": format!("{:?}", result) }]
                            }
                        })),
                        None => Ok(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32603, "message": format!("Plugin not found: {}", pid) }
                        })),
                    }
                } else {
                    Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("{:?}", health_results) }]
                        }
                    }))
                }
            }
            "plugin_update_check" => {
                let updates = self.plugin_manager.check_for_updates();
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("{:?}", updates) }]
                    }
                }))
            }
            "plugin_cleanup" => {
                let max_age_seconds = args["max_age_seconds"].as_i64().unwrap_or(86400);
                let removed = self.plugin_manager.cleanup_unused_plugins(max_age_seconds);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Removed plugins: {:?}", removed) }]
                    }
                }))
            }
            "connection_status" => {
                let mut connections = self.connections.lock().unwrap();
                // Clean up stale connections (older than 5 minutes)
                let mut to_remove = Vec::new();
                for (id, conn) in connections.iter() {
                    if conn.last_activity.elapsed() > Duration::from_secs(300) {
                        to_remove.push(id.clone());
                    }
                }
                for id in to_remove {
                    connections.remove(&id);
                }
                // Build status list
                let mut status_list = Vec::new();
                for (id, conn) in connections.iter_mut() {
                    // Update status based on activity
                    let elapsed = conn.last_activity.elapsed();
                    let new_status = if elapsed < Duration::from_secs(30) {
                        ConnectionStatus::Connected
                    } else {
                        ConnectionStatus::Idle
                    };
                    conn.status = new_status;
                    let status_str = match conn.status {
                        ConnectionStatus::Connected => "🟢 Connected",
                        ConnectionStatus::Idle => "🟡 Idle",
                        ConnectionStatus::Disconnected => "🔴 Disconnected",
                    };
                    let duration = elapsed.as_secs();
                    status_list.push(format!(
                        "{} ({}): {} - {}s ago via {}",
                        conn.client_name, id, status_str, duration, conn.protocol
                    ));
                }
                let output = if status_list.is_empty() {
                    "No active connections".to_string()
                } else {
                    status_list.join("\n")
                };
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": output }]
                    }
                }))
            }
            _ => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": "Unknown tool" }]
                }
            })),
        }
    }
}
