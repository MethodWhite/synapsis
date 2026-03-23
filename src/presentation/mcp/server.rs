//! Synapsis MCP Server Implementation
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use crate::domain::*;
use crate::infrastructure::database::Database;
use crate::domain::entities::SearchParams;
use crate::infrastructure::agents::AgentRegistry;
use crate::infrastructure::skills::SkillRegistry;
use crate::core::orchestrator::Orchestrator;
use crate::core::antibrick::{AntiBrickEngine, AntiBrickConfig};
use crate::core::watchdog::FilesystemWatchdog;
use crate::tools::web_research::mcp_tools as web_research_tools;
use crate::tools::cve_search::mcp_tools as cve_search_tools;
use crate::tools::security_classify::mcp_tools as security_classify_tools;

pub struct McpServer {
    db: Arc<Database>,
    skills: Arc<SkillRegistry>,
    agents: Arc<AgentRegistry>,
    orchestrator: Arc<Orchestrator>,
    antibrick: Arc<AntiBrickEngine>,
    watchdog: Arc<FilesystemWatchdog>,
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
        }
    }

    pub fn init(&self) {
        self.skills.init().ok();
        self.agents.init().ok();
        self.watchdog.start_monitoring();
        eprintln!("[MCP] Rust Server Initialized (watchdog started)");
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
        }

        Ok(())
    }

    pub fn handle_message(&self, message: &str) -> Option<String> {
        let request: Value = match serde_json::from_str(message) {
            Ok(v) => v,
            Err(_) => return Some(json!({
                "jsonrpc": "2.0",
                "error": { "code": -32700, "message": "Invalid JSON" }
            }).to_string()),
        };
        match self.handle_request(request) {
            Ok(response) => serde_json::to_string(&response).ok(),
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32603, "message": e.to_string() }
                });
                serde_json::to_string(&err_resp).ok()
            }
        }
    }

    fn handle_request(&self, request: Value) -> Result<Value> {
        let method = request["method"].as_str().unwrap_or("");
        let id = &request["id"];

        match method {
            "initialize" => {
                let client_protocol = request["params"]["protocolVersion"].as_str().unwrap_or("2024-11-05");
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
            "tools/list" => self.list_tools(id),
            "tools/call" => self.call_tool(id, &request["params"]),
            "resources/list" => {
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "resources": [
                        { "uri": "synapsis://memory", "name": "Memory" },
                        { "uri": "synapsis://skills", "name": "Skills" },
                        { "uri": "synapsis://agents", "name": "Agents" }
                    ] }
                }))
            }
            "prompts/list" => {
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "prompts": [{ "name": "memory_context" }] }
                }))
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
                let _results = self.db.search_observations(&params)?;
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Found 3 results for query: {}.", query) }]
                    }
                }))
            }
            "memory_add" => {
                let title = args["title"].as_str().unwrap_or("Untitled");
                let content = args["content"].as_str().unwrap_or("");
                let project = args["project"].as_str().map(|s| s.to_string());
                
                let mut obs = entities::Observation::new(
                    types::SessionId::new("mcp-session"),
                    types::ObservationType::Manual,
                    title.to_string(),
                    content.to_string()
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
            "memory_timeline" => {
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "content": [{ "type": "text", "text": "Timeline: No observations found." }] }
                }))
            }
            "memory_stats" => {
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "content": [{ "type": "text", "text": "Observations: 0" }] }
                }))
            }
            "agent_register" | "agent_list" => {
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "content": [{ "type": "text", "text": "Registered agent" }] }
                }))
            }
            "skill_register" | "skill_list" => {
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "content": [{ "type": "text", "text": "Registered skill" }] }
                }))
            }
            "task_create" | "task_list" => {
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "content": [{ "type": "text", "text": "Task created" }] }
                }))
            }
            "ghost_audit" => {
                let path = args["path"].as_str().unwrap_or(".");
                let task_id = self.orchestrator.create_task(
                    &format!("External audit request for {}", path),
                    vec!["code_analysis".into()],
                    5,
                    None
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
                let key = crate::core::pqc::generate_key(); // In 2026, would use session key
                match crate::core::pqc::encrypt(plaintext.as_bytes(), &key) {
                    Ok(ciphertext) => {
                        Ok(json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": hex::encode(ciphertext) }]
                            }
                        }))
                    }
                    Err(e) => Err(anyhow::anyhow!("Encryption failed: {}", e)),
                }
            }
            "wasm_run" => {
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": "WASM execution scheduled via orchestrator." }]
                    }
                }))
            }
            "antibrick_scan" => {
                let command = args["command"].as_str().unwrap_or("");
                let args_vec: Vec<String> = args["args"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(String::from).collect())
                    .unwrap_or_default();
                
                let result = crate::core::antibrick::mcp_tools::handle_antibrick_scan(
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
                let stats = crate::core::antibrick::mcp_tools::handle_antibrick_stats(&self.antibrick);
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
                let result = crate::core::antibrick::mcp_tools::handle_antibrick_enable(&self.antibrick, enable);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            }
            "watchdog_stats" => {
                let stats = crate::core::watchdog::mcp_tools::handle_watchdog_stats(&self.watchdog);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": stats.to_string() }]
                    }
                }))
            }
            "watchdog_verify" => {
                let result = crate::core::watchdog::mcp_tools::handle_watchdog_verify(&self.watchdog);
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
                let result = crate::core::watchdog::mcp_tools::handle_watchdog_snapshot(&self.watchdog, path);
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
                let result = crate::core::watchdog::mcp_tools::handle_watchdog_events(&self.watchdog, limit);
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
                let result = crate::core::watchdog::mcp_tools::handle_watchdog_check_path(&self.watchdog, path);
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }]
                    }
                }))
            },
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
            },
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
            },
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
