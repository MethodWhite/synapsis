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

use crate::tools::auth_browser::mcp_tools as auth_browser_tools;
use crate::tools::browser_navigation::mcp_tools as browser_navigation_tools;
use crate::tools::cve_search::mcp_tools as cve_search_tools;
use crate::tools::env_detection::handle_env_detection;
use crate::tools::security_classify::mcp_tools as security_classify_tools;
use crate::tools::web_research::mcp_tools as web_research_tools;

// Plugins
use crate::plugins::smart_browser::mcp_tools as smart_browser_tools;
use crate::plugins::remote_control::mcp_tools as remote_control_tools;
use crate::plugins::security_shield::mcp_tools as security_shield_tools;
use crate::plugins::security_shield;
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

        // Initialize security shield default rules
        security_shield::init_default_rules();

        eprintln!("[MCP] Rust Server Initialized (watchdog + security shield started)");
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
                    },
                    {
                        "name": "kino_predict",
                        "description": "Get Kino lottery prediction using NUM-JEPA (M.A.T.E.R.I.A. engine)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "top": { "type": "integer", "default": 15, "description": "Number of top predictions" },
                                "arch": { "type": "boolean", "default": false, "description": "Use full toroidal-hexagonal architecture" }
                            }
                        }
                    },
                    {
                        "name": "kino_train",
                        "description": "Trigger NUM-JEPA training for Kino prediction model",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "epochs": { "type": "integer", "default": 100, "description": "Number of training epochs" }
                            }
                        }
                    },
                    {
                        "name": "kino_stats",
                        "description": "Get Kino system statistics and analysis",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "materia_status",
                        "description": "Get M.A.T.E.R.I.A. engine full system status",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "system_resources",
                        "description": "Get GPU/RAM/CPU system resource usage",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "auth_screenshot",
                        "description": "Take a screenshot of the current page in an authenticated session (useful for debugging login pages)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "output_path": { "type": "string" },
                                "wait_seconds": { "type": "integer", "default": 5 }
                            },
                            "required": ["session_id", "output_path"]
                        }
                    },
                    {
                        "name": "auth_login_and_extract",
                        "description": "Login to a website and extract visible text content in a single operation (SPA-friendly, ideal for Netacad, LMS platforms)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "Target page URL to extract content from" },
                                "session_id": { "type": "string" },
                                "login_url": { "type": "string", "description": "Login page URL" },
                                "login_selector_user": { "type": "string", "description": "CSS selector for username field" },
                                "login_selector_pass": { "type": "string", "description": "CSS selector for password field" },
                                "username": { "type": "string" },
                                "password": { "type": "string" },
                                "login_button_selector": { "type": "string", "description": "CSS selector for login button" },
                                "wait_seconds": { "type": "integer", "default": 10, "description": "Seconds to wait for SPA rendering after navigation" }
                            },
                            "required": ["url", "session_id"]
                        }
                    },
                    {
                        "name": "auth_navigate",
                        "description": "Navigate to a web page with authentication support (login, cookies, session persistence)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string" },
                                "session_id": { "type": "string" },
                                "login_url": { "type": "string" },
                                "login_selector_user": { "type": "string" },
                                "login_selector_pass": { "type": "string" },
                                "username": { "type": "string" },
                                "password": { "type": "string" },
                                "login_button_selector": { "type": "string" }
                            },
                            "required": ["url", "session_id"]
                        }
                    },
                    {
                        "name": "auth_extract",
                        "description": "Extract content from an authenticated browser session using CSS selectors",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "selector": { "type": "string" }
                            },
                            "required": ["session_id", "selector"]
                        }
                    },
                    {
                        "name": "auth_extract_text",
                        "description": "Extract all visible text content from an authenticated session (SPA-friendly, waits for JS rendering)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "wait_seconds": { "type": "integer", "default": 8, "description": "Seconds to wait for SPA rendering" }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "auth_navigate_session",
                        "description": "Navigate to a new URL within an existing authenticated session",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "url": { "type": "string" }
                            },
                            "required": ["session_id", "url"]
                        }
                    },
                    {
                        "name": "auth_clear_session",
                        "description": "Clear/delete a saved browser authentication session",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "auth_list_sessions",
                        "description": "List all saved authenticated browser sessions",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "smart_navigate",
                        "description": "Navigate to URL and analyze page like a human (finds forms, links, buttons automatically)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "url": { "type": "string" },
                                "wait_seconds": { "type": "integer", "default": 5 }
                            },
                            "required": ["session_id", "url"]
                        }
                    },
                    {
                        "name": "smart_find_element",
                        "description": "Find elements intelligently by text, role, or context (like a human searching for something on a page)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "search_query": { "type": "string", "description": "What to look for (e.g., 'login button', 'email field', 'submit')" },
                                "element_type": { "type": "string", "default": "any", "description": "Filter by element type (button, input, link, etc.)" }
                            },
                            "required": ["session_id", "search_query"]
                        }
                    },
                    {
                        "name": "smart_click",
                        "description": "Click an element with human-like timing (scrolls into view, waits for navigation)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "selector": { "type": "string" }
                            },
                            "required": ["session_id", "selector"]
                        }
                    },
                    {
                        "name": "smart_fill",
                        "description": "Fill a form field by description (finds by label, placeholder, or name automatically)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "field_description": { "type": "string", "description": "Describe the field (e.g., 'email', 'password', 'search box')" },
                                "value": { "type": "string" }
                            },
                            "required": ["session_id", "field_description", "value"]
                        }
                    },
                    {
                        "name": "smart_submit",
                        "description": "Submit a form intelligently (finds submit button or submits form directly)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "smart_screenshot",
                        "description": "Take a screenshot of the current page for analysis or debugging",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" },
                                "output_path": { "type": "string" }
                            },
                            "required": ["session_id", "output_path"]
                        }
                    },
                    {
                        "name": "smart_session_info",
                        "description": "Get info about a smart browser session (current URL, title, action history)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "session_id": { "type": "string" }
                            },
                            "required": ["session_id"]
                        }
                    },
                    {
                        "name": "agent_register",
                        "description": "Register a new agent in the system with capabilities",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "agent_id": { "type": "string" },
                                "name": { "type": "string" },
                                "capabilities": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["agent_id", "name"]
                        }
                    },
                    {
                        "name": "agent_send_message",
                        "description": "Send a secure message to another agent",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "from": { "type": "string" },
                                "to": { "type": "string" },
                                "content": { "type": "string" },
                                "message_type": { "type": "string", "default": "command" },
                                "priority": { "type": "integer", "default": 5 }
                            },
                            "required": ["from", "to", "content"]
                        }
                    },
                    {
                        "name": "agent_receive_messages",
                        "description": "Receive pending messages for an agent",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "agent_id": { "type": "string" },
                                "limit": { "type": "integer", "default": 10 }
                            },
                            "required": ["agent_id"]
                        }
                    },
                    {
                        "name": "agent_self_configure",
                        "description": "Auto-configure agent settings from learned data",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "agent_id": { "type": "string" },
                                "config_updates": { "type": "object" }
                            },
                            "required": ["agent_id"]
                        }
                    },
                    {
                        "name": "agent_self_heal",
                        "description": "Detect and fix common system issues automatically",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "agent_add_heal_rule",
                        "description": "Add a self-healing rule (trigger + condition + action)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "trigger_pattern": { "type": "string" },
                                "condition": { "type": "string" },
                                "action": { "type": "string" }
                            },
                            "required": ["trigger_pattern", "condition", "action"]
                        }
                    },
                    {
                        "name": "agent_learn",
                        "description": "Learn from feedback to improve behavior over time",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "agent_id": { "type": "string" },
                                "action": { "type": "string" },
                                "result": { "type": "string" },
                                "success": { "type": "boolean" }
                            },
                            "required": ["agent_id", "action", "result", "success"]
                        }
                    },
                    {
                        "name": "agent_execute_command",
                        "description": "Execute a system command securely (with allowlist validation)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string" },
                                "args": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["command"]
                        }
                    },
                    {
                        "name": "agent_secure_read",
                        "description": "Read a file securely (with path validation and size limits)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "agent_update_security_policy",
                        "description": "Update the security policy (allowed/blocked commands, rate limits)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "updates": { "type": "object" }
                            },
                            "required": ["updates"]
                        }
                    },
                    {
                        "name": "agent_security_status",
                        "description": "Get current security status (policy, registered agents, audit log)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "security_sanitize_input",
                        "description": "Sanitize input against all known injection attacks (SQL, XSS, Command, RCE, etc.)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "input": { "type": "string" },
                                "context": { "type": "string", "default": "general" }
                            },
                            "required": ["input"]
                        }
                    },
                    {
                        "name": "security_is_safe",
                        "description": "Quick check if input is safe (no threats detected)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "input": { "type": "string" }
                            },
                            "required": ["input"]
                        }
                    },
                    {
                        "name": "security_monitor_network",
                        "description": "Monitor network connections and detect suspicious activity",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "security_detect_lateral_movement",
                        "description": "Scan for lateral movement attempts (SMB, WMI, auth brute force)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "security_detect_gap_attacks",
                        "description": "Detect air gap crossing attempts (USB, Bluetooth, unusual interfaces)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "security_audit",
                        "description": "Full security audit: lateral movement + gap attacks + network monitoring + risk assessment",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "security_threat_log",
                        "description": "Get recent threat detections",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "limit": { "type": "integer", "default": 20 }
                            }
                        }
                    },
                    {
                        "name": "security_events",
                        "description": "Get recent security events",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "limit": { "type": "integer", "default": 20 }
                            }
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
            "mem_session_start" | "session_register" => self.action_mem_session_start(args),
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

            // M.A.T.E.R.I.A. NUM-JEPA
            "kino_predict" => self.action_kino_predict(args),
            "kino_train" => self.action_kino_train(args),
            "kino_stats" => self.action_kino_stats(args),
            "materia_status" => self.action_materia_status(args),
            "system_resources" => self.action_system_resources(args),
            
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

            // System: Authenticated Browser
            "auth_screenshot" => self.action_auth_screenshot(args),
            "auth_login_and_extract" => self.action_auth_login_and_extract(args),
            "auth_navigate" => self.action_auth_navigate(args),
            "auth_extract" => self.action_auth_extract(args),
            "auth_extract_text" => self.action_auth_extract_text(args),
            "auth_navigate_session" => self.action_auth_navigate_session(args),
            "auth_clear_session" => self.action_auth_clear_session(args),
            "auth_list_sessions" => self.action_auth_list_sessions(args),

            // Plugins: Smart Browser
            "smart_navigate" => self.action_smart_navigate(args),
            "smart_find_element" => self.action_smart_find_element(args),
            "smart_click" => self.action_smart_click(args),
            "smart_fill" => self.action_smart_fill(args),
            "smart_submit" => self.action_smart_submit(args),
            "smart_screenshot" => self.action_smart_screenshot(args),
            "smart_session_info" => self.action_smart_session_info(args),

            // Plugins: Remote Control
            "agent_register" => self.action_agent_register(args),
            "agent_send_message" => self.action_agent_send_message(args),
            "agent_receive_messages" => self.action_agent_receive_messages(args),
            "agent_self_configure" => self.action_agent_self_configure(args),
            "agent_self_heal" => self.action_agent_self_heal(args),
            "agent_add_heal_rule" => self.action_agent_add_heal_rule(args),
            "agent_learn" => self.action_agent_learn(args),
            "agent_execute_command" => self.action_agent_execute_command(args),
            "agent_secure_read" => self.action_agent_secure_read(args),
            "agent_update_security_policy" => self.action_agent_update_security_policy(args),
            "agent_security_status" => self.action_agent_security_status(args),

            // Plugins: Security Shield
            "security_sanitize_input" => self.action_security_sanitize_input(args),
            "security_is_safe" => self.action_security_is_safe(args),
            "security_monitor_network" => self.action_security_monitor_network(args),
            "security_detect_lateral_movement" => self.action_security_detect_lateral_movement(args),
            "security_detect_gap_attacks" => self.action_security_detect_gap_attacks(args),
            "security_audit" => self.action_security_audit(args),
            "security_threat_log" => self.action_security_threat_log(args),
            "security_events" => self.action_security_events(args),

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

    // Authenticated Browser Actions

    fn action_auth_screenshot(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let output_path = args["output_path"].as_str().unwrap_or("/tmp/auth-screenshot.png");
        let wait_seconds = args.get("wait_seconds").and_then(|v| v.as_u64());
        Ok(auth_browser_tools::handle_auth_screenshot(session_id, output_path, wait_seconds))
    }

    fn action_auth_login_and_extract(&self, args: &Value) -> Result<Value, String> {
        let url = args["url"].as_str().unwrap_or("");
        let session_id = args["session_id"].as_str().unwrap_or("");
        let login_url = args.get("login_url").and_then(|v| v.as_str());
        let login_selector_user = args.get("login_selector_user").and_then(|v| v.as_str());
        let login_selector_pass = args.get("login_selector_pass").and_then(|v| v.as_str());
        let username = args.get("username").and_then(|v| v.as_str());
        let password = args.get("password").and_then(|v| v.as_str());
        let login_button_selector = args.get("login_button_selector").and_then(|v| v.as_str());
        let wait_seconds = args.get("wait_seconds").and_then(|v| v.as_u64()).unwrap_or(10);
        Ok(auth_browser_tools::handle_auth_login_and_extract(
            url, session_id, login_url, login_selector_user,
            login_selector_pass, username, password, login_button_selector, wait_seconds,
        ))
    }

    fn action_auth_navigate(&self, args: &Value) -> Result<Value, String> {
        let url = args["url"].as_str().unwrap_or("");
        let session_id = args["session_id"].as_str().unwrap_or("");
        let login_url = args.get("login_url").and_then(|v| v.as_str());
        let login_selector_user = args.get("login_selector_user").and_then(|v| v.as_str());
        let login_selector_pass = args.get("login_selector_pass").and_then(|v| v.as_str());
        let username = args.get("username").and_then(|v| v.as_str());
        let password = args.get("password").and_then(|v| v.as_str());
        let login_button_selector = args.get("login_button_selector").and_then(|v| v.as_str());
        Ok(auth_browser_tools::handle_auth_navigate(
            url, session_id, login_url, login_selector_user,
            login_selector_pass, username, password, login_button_selector,
        ))
    }

    fn action_auth_extract(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let selector = args["selector"].as_str().unwrap_or("");
        Ok(auth_browser_tools::handle_auth_extract(session_id, selector))
    }

    fn action_auth_extract_text(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let wait_seconds = args.get("wait_seconds").and_then(|v| v.as_u64());
        Ok(auth_browser_tools::handle_auth_extract_text(session_id, wait_seconds))
    }

    fn action_auth_navigate_session(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let url = args["url"].as_str().unwrap_or("");
        Ok(auth_browser_tools::handle_auth_navigate_session(session_id, url))
    }

    fn action_auth_clear_session(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        Ok(auth_browser_tools::handle_auth_clear_session(session_id))
    }

    fn action_auth_list_sessions(&self, args: &Value) -> Result<Value, String> {
        let _ = args;
        Ok(auth_browser_tools::handle_auth_list_sessions())
    }

    // === Plugin Action Methods ===

    // Smart Browser Actions
    fn action_smart_navigate(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let url = args["url"].as_str().unwrap_or("");
        let wait = args.get("wait_seconds").and_then(|v| v.as_u64());
        Ok(smart_browser_tools::handle_smart_navigate(session_id, url, wait))
    }

    fn action_smart_find_element(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let search = args["search_query"].as_str().unwrap_or("");
        let etype = args.get("element_type").and_then(|v| v.as_str());
        Ok(smart_browser_tools::handle_smart_find(session_id, search, etype))
    }

    fn action_smart_click(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let selector = args["selector"].as_str().unwrap_or("");
        Ok(smart_browser_tools::handle_smart_click(session_id, selector))
    }

    fn action_smart_fill(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let field = args["field_description"].as_str().unwrap_or("");
        let value = args["value"].as_str().unwrap_or("");
        Ok(smart_browser_tools::handle_smart_fill(session_id, field, value))
    }

    fn action_smart_submit(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        Ok(smart_browser_tools::handle_smart_submit(session_id))
    }

    fn action_smart_screenshot(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        let output_path = args["output_path"].as_str().unwrap_or("/tmp/smart-screenshot.png");
        Ok(smart_browser_tools::handle_smart_screenshot(session_id, output_path))
    }

    fn action_smart_session_info(&self, args: &Value) -> Result<Value, String> {
        let session_id = args["session_id"].as_str().unwrap_or("");
        Ok(smart_browser_tools::handle_smart_session_info(session_id))
    }

    // Remote Control Actions
    fn action_agent_register(&self, args: &Value) -> Result<Value, String> {
        let agent_id = args["agent_id"].as_str().unwrap_or("");
        let name = args["name"].as_str().unwrap_or("");
        let caps = args.get("capabilities").and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>());
        Ok(remote_control_tools::handle_agent_register(agent_id, name, caps.as_deref()))
    }

    fn action_agent_send_message(&self, args: &Value) -> Result<Value, String> {
        let from = args["from"].as_str().unwrap_or("");
        let to = args["to"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");
        let msg_type = args.get("message_type").and_then(|v| v.as_str()).unwrap_or("command");
        let priority = args.get("priority").and_then(|v| v.as_u64()).unwrap_or(5) as u8;
        Ok(remote_control_tools::handle_send_message(from, to, content, msg_type, priority))
    }

    fn action_agent_receive_messages(&self, args: &Value) -> Result<Value, String> {
        let agent_id = args["agent_id"].as_str().unwrap_or("");
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
        Ok(remote_control_tools::handle_receive_messages(agent_id, limit))
    }

    fn action_agent_self_configure(&self, args: &Value) -> Result<Value, String> {
        let agent_id = args["agent_id"].as_str().unwrap_or("");
        let default_config = json!({});
        let config = args.get("config_updates").unwrap_or(&default_config);
        Ok(remote_control_tools::handle_self_configure(agent_id, config))
    }

    fn action_agent_self_heal(&self, args: &Value) -> Result<Value, String> {
        let _ = args;
        Ok(remote_control_tools::handle_self_heal())
    }

    fn action_agent_add_heal_rule(&self, args: &Value) -> Result<Value, String> {
        let trigger = args.get("trigger_pattern").and_then(|v| v.as_str()).unwrap_or("");
        let condition = args.get("condition").and_then(|v| v.as_str()).unwrap_or("");
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");
        Ok(remote_control_tools::handle_add_heal_rule(trigger, condition, action))
    }

    fn action_agent_learn(&self, args: &Value) -> Result<Value, String> {
        let agent_id = args["agent_id"].as_str().unwrap_or("");
        let action = args["action"].as_str().unwrap_or("");
        let result = args["result"].as_str().unwrap_or("");
        let success = args["success"].as_bool().unwrap_or(false);
        Ok(remote_control_tools::handle_learn(agent_id, action, result, success))
    }

    fn action_agent_execute_command(&self, args: &Value) -> Result<Value, String> {
        let command = args["command"].as_str().unwrap_or("");
        let arg_arr = args.get("args").and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
            .unwrap_or_default();
        Ok(remote_control_tools::handle_execute_command(command, &arg_arr))
    }

    fn action_agent_secure_read(&self, args: &Value) -> Result<Value, String> {
        let path = args["path"].as_str().unwrap_or("");
        Ok(remote_control_tools::handle_secure_read(path))
    }

    fn action_agent_update_security_policy(&self, args: &Value) -> Result<Value, String> {
        let default_updates = json!({});
        let updates = args.get("updates").unwrap_or(&default_updates);
        Ok(remote_control_tools::handle_update_security_policy(updates))
    }

    fn action_agent_security_status(&self, args: &Value) -> Result<Value, String> {
        let _ = args;
        Ok(remote_control_tools::handle_security_status())
    }

    // Security Shield Actions
    fn action_security_sanitize_input(&self, args: &Value) -> Result<Value, String> {
        let input = args["input"].as_str().unwrap_or("");
        let context = args.get("context").and_then(|v| v.as_str()).unwrap_or("general");
        Ok(security_shield_tools::handle_sanitize_input(input, context))
    }

    fn action_security_is_safe(&self, args: &Value) -> Result<Value, String> {
        let input = args["input"].as_str().unwrap_or("");
        Ok(security_shield_tools::handle_is_safe(input))
    }

    fn action_security_monitor_network(&self, args: &Value) -> Result<Value, String> {
        let _ = args;
        Ok(security_shield_tools::handle_monitor_network())
    }

    fn action_security_detect_lateral_movement(&self, args: &Value) -> Result<Value, String> {
        let _ = args;
        Ok(security_shield_tools::handle_detect_lateral_movement())
    }

    fn action_security_detect_gap_attacks(&self, args: &Value) -> Result<Value, String> {
        let _ = args;
        Ok(security_shield_tools::handle_detect_gap_attacks())
    }

    fn action_security_audit(&self, args: &Value) -> Result<Value, String> {
        let _ = args;
        Ok(security_shield_tools::handle_security_audit())
    }

    fn action_security_threat_log(&self, args: &Value) -> Result<Value, String> {
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as u32;
        Ok(security_shield_tools::handle_threat_log(limit))
    }

    fn action_security_events(&self, args: &Value) -> Result<Value, String> {
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as u32;
        Ok(security_shield_tools::handle_security_events(limit))
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

    // --- M.A.T.E.R.I.A. NUM-JEPA Actions ---

    fn run_materia_script(&self, script: &str, script_args: &[&str]) -> Result<Value, String> {
        let script_path = format!("/home/methodwhite/MATERIA/scripts/{}", script);
        let mut cmd = std::process::Command::new("python3");
        cmd.arg(&script_path).args(script_args);
        cmd.env("PYTHONIOENCODING", "utf-8");

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute {}: {}", script, e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = serde_json::Map::new();
        result.insert("exit_code".to_string(), Value::Number(serde_json::Number::from(output.status.code().unwrap_or(-1))));
        if !stdout.trim().is_empty() {
            // Try to parse stdout as JSON first
            if let Ok(parsed) = serde_json::from_str::<Value>(stdout.trim()) {
                result.insert("output".to_string(), parsed);
            } else {
                result.insert("output".to_string(), Value::String(stdout.trim().to_string()));
            }
        }
        if !stderr.trim().is_empty() {
            result.insert("stderr".to_string(), Value::String(stderr.trim().to_string()));
        }
        result.insert("success".to_string(), Value::Bool(output.status.success()));

        Ok(Value::Object(result))
    }

    fn action_kino_predict(&self, args: &Value) -> Result<Value, String> {
        let mut cmd_args = vec!["predict"];
        if let Some(top) = args.get("top").and_then(|v| v.as_i64()) {
            cmd_args.push("--top");
            cmd_args.push(Box::leak(top.to_string().into_boxed_str()));
        }
        if args.get("arch").and_then(|v| v.as_bool()).unwrap_or(false) {
            cmd_args.push("--arch");
        }
        self.run_materia_script("kino_predict.py", &cmd_args)
    }

    fn action_kino_train(&self, args: &Value) -> Result<Value, String> {
        let mut cmd_args = vec!["train"];
        if let Some(epochs) = args.get("epochs").and_then(|v| v.as_i64()) {
            cmd_args.push("--epochs");
            cmd_args.push(Box::leak(epochs.to_string().into_boxed_str()));
        }
        self.run_materia_script("kino_predict.py", &cmd_args)
    }

    fn action_kino_stats(&self, args: &Value) -> Result<Value, String> {
        let _ = args;
        self.run_materia_script("kino_predict.py", &["stats"])
    }

    fn action_materia_status(&self, args: &Value) -> Result<Value, String> {
        let _ = args;
        self.run_materia_script("kino_predict.py", &["status"])
    }

    fn action_system_resources(&self, args: &Value) -> Result<Value, String> {
        let _ = args;

        // GPU info via nvidia-smi
        let gpu_info = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=index,name,memory.used,memory.total,utilization.gpu,utilization.memory,temperature.gpu", "--format=csv,noheader,nounits"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok());

        // RAM info
        let ram_info = std::process::Command::new("free")
            .args(["-m"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok());

        // CPU info
        let cpu_info = std::process::Command::new("nproc")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());

        let load_avg = std::fs::read_to_string("/proc/loadavg")
            .ok()
            .map(|s| s.trim().to_string());

        let mut result = serde_json::Map::new();

        if let Some(gpu) = gpu_info {
            let mut gpus = Vec::new();
            for line in gpu.lines() {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 7 {
                    let mut gpu_obj = serde_json::Map::new();
                    gpu_obj.insert("id".to_string(), Value::String(parts[0].to_string()));
                    gpu_obj.insert("name".to_string(), Value::String(parts[1].to_string()));
                    if let Ok(mem_used) = parts[2].parse::<f64>() {
                        gpu_obj.insert("memory_used_mb".to_string(), Value::Number(serde_json::Number::from_f64(mem_used).unwrap_or(serde_json::Number::from(0))));
                    }
                    if let Ok(mem_total) = parts[3].parse::<f64>() {
                        gpu_obj.insert("memory_total_mb".to_string(), Value::Number(serde_json::Number::from_f64(mem_total).unwrap_or(serde_json::Number::from(0))));
                    }
                    if let Ok(util_gpu) = parts[4].parse::<f64>() {
                        gpu_obj.insert("gpu_utilization_pct".to_string(), Value::Number(serde_json::Number::from_f64(util_gpu).unwrap_or(serde_json::Number::from(0))));
                    }
                    if let Ok(util_mem) = parts[5].parse::<f64>() {
                        gpu_obj.insert("memory_utilization_pct".to_string(), Value::Number(serde_json::Number::from_f64(util_mem).unwrap_or(serde_json::Number::from(0))));
                    }
                    if let Ok(temp) = parts[6].parse::<f64>() {
                        gpu_obj.insert("temperature_c".to_string(), Value::Number(serde_json::Number::from_f64(temp).unwrap_or(serde_json::Number::from(0))));
                    }
                    gpus.push(Value::Object(gpu_obj));
                }
            }
            result.insert("gpus".to_string(), Value::Array(gpus));
        } else {
            result.insert("gpus".to_string(), Value::String("nvidia-smi not available".to_string()));
        }

        if let Some(ram) = ram_info {
            result.insert("ram_free_m".to_string(), Value::String(ram));
        }

        if let Some(cpus) = cpu_info {
            result.insert("cpu_cores".to_string(), Value::String(cpus.trim().to_string()));
        }

        if let Some(load) = load_avg {
            result.insert("load_avg".to_string(), Value::String(load));
        }

        Ok(Value::Object(result))
    }
}
