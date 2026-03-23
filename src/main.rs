//! Synapsis - Unified MCP Server
//!
//! Single MCP server that handles both stdio (local) and TCP (remote) connections.
//! No duplication - one state, multiple transports.
//!
//! Sovereign stack: Self-contained PQC, multi-agent orchestration, skills.

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Instant;

use synapsis::core::auth::challenge::ChallengeResponse;
use synapsis::core::auth::classifier::AgentClassifier;
use synapsis::core::auth::tpm::TpmMfaProvider;
use synapsis::core::auto_integrate::AutoIntegrate;
use synapsis::core::discovery::EnvironmentDiscovery;
use synapsis::core::orchestrator::{AgentStatus, Orchestrator};
use synapsis::core::recycle::bin::RecycleBin;
use synapsis::core::tool_registry::ToolRegistry;
use synapsis::core::uuid::Uuid;
use synapsis::core::rate_limiter::RateLimiter;
use synapsis::core::vault::SecureVault;
use synapsis::infrastructure::shared_state::SharedState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

struct EventBus {
    events: Arc<Mutex<Vec<Event>>>,
    message_queue: Arc<Mutex<std::collections::HashMap<String, Vec<PendingMessage>>>>,
}

impl EventBus {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            message_queue: Arc::new(Mutex::new(std::collections::HashMap::new())),
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
        queue
            .remove(session_id)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Skill {
    id: String,
    name: String,
    category: String,
    description: String,
    instructions: String,
    enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AgentSession {
    agent_type: String,
    instance: String,
    session_id: String,
    project: String,
    last_seen: i64,
    auto_reconnect: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Worker {
    id: String,
    agent: String,
    status: String,
    capabilities: Vec<String>,
    current_task: Option<String>,
    registered_at: i64,
    last_heartbeat: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DelegatedTask {
    id: String,
    description: String,
    payload: String,
    priority: u8,
    required_skills: Vec<String>,
    status: String,
    assigned_worker: Option<String>,
    result: Option<String>,
    created_at: i64,
    assigned_at: Option<i64>,
    completed_at: Option<i64>,
}

struct ServerState {
    shared: Arc<SharedState>,
    sessions: Arc<Mutex<Vec<AgentSession>>>,
    skills: Arc<Mutex<Vec<Skill>>>,
    data_dir: PathBuf,
    event_bus: Arc<EventBus>,
    active_connections: Arc<Mutex<Vec<ConnectionInfo>>>,
    orchestrator: Arc<Orchestrator>,
    classifier: Arc<AgentClassifier>,
    challenge_response: Arc<ChallengeResponse>,
    tpm_mfa: Arc<TpmMfaProvider>,
    recycle_bin: Arc<RecycleBin>,
    vault: Arc<SecureVault>,
    workers: Arc<Mutex<Vec<Worker>>>,
    delegated_tasks: Arc<Mutex<Vec<DelegatedTask>>>,
    event_subscriptions: Arc<Mutex<std::collections::HashMap<String, String>>>,
    discovery: Arc<EnvironmentDiscovery>,
    tool_registry: Arc<RwLock<ToolRegistry>>,
    auto_integrate: Arc<Mutex<Option<AutoIntegrate>>>,
    rate_limiter: RateLimiter,
}

#[derive(Debug, Clone)]
struct ConnectionInfo {
    session_id: String,
    agent_type: String,
    project: String,
    last_activity: Instant,
}

impl ServerState {
    fn new() -> Self {
        let data_dir = dirs_data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapsis");
        fs::create_dir_all(&data_dir).ok();

        let shared = Arc::new(SharedState::new());
        shared.init();

        let skills = Self::load_skills(&data_dir);
        let sessions = Self::load_sessions(&data_dir);

        let orchestrator = Orchestrator::new_with_persistence(&data_dir);

        let classifier = Arc::new(AgentClassifier::new());
        let challenge_response = Arc::new(ChallengeResponse::new());
        let tpm_mfa = Arc::new(TpmMfaProvider::new());

        let recycle_bin = Arc::new(RecycleBin::new(data_dir.clone()));
        recycle_bin.load().ok();

        let vault = Arc::new(SecureVault::new(data_dir.clone()));
        vault.initialize().ok();

        Self {
            shared,
            skills: Arc::new(Mutex::new(skills)),
            sessions: Arc::new(Mutex::new(sessions)),
            data_dir,
            event_bus: Arc::new(EventBus::new()),
            active_connections: Arc::new(Mutex::new(Vec::new())),
            orchestrator: Arc::new(orchestrator),
            classifier,
            challenge_response,
            tpm_mfa,
            recycle_bin,
            vault,
            workers: Arc::new(Mutex::new(Vec::new())),
            delegated_tasks: Arc::new(Mutex::new(Vec::new())),
            event_subscriptions: Arc::new(Mutex::new(std::collections::HashMap::new())),
            discovery: Arc::new(EnvironmentDiscovery::new()),
            tool_registry: Arc::new(RwLock::new(ToolRegistry::new())),
            auto_integrate: Arc::new(Mutex::new(None)),
            rate_limiter: RateLimiter::new(10, 100), // 10 requests per second, burst up to 100
        }
    }

    fn load_skills(data_dir: &Path) -> Vec<Skill> {
        let skills_file = data_dir.join("skills.json");
        if let Ok(data) = fs::read_to_string(&skills_file) {
            if let Ok(skills) = serde_json::from_str(&data) {
                return skills;
            }
        }
        Self::default_skills()
    }

    fn load_sessions(data_dir: &Path) -> Vec<AgentSession> {
        let sessions_file = data_dir.join("sessions.json");
        if let Ok(data) = fs::read_to_string(&sessions_file) {
            if let Ok(sessions) = serde_json::from_str(&data) {
                return sessions;
            }
        }
        Vec::new()
    }

    fn _save_skills(&self) {
        if let Ok(skills) = fs::read_to_string(self.data_dir.join("skills.json")) {
            let _ = fs::write(self.data_dir.join("skills.json"), skills);
        }
    }

    fn save_sessions(&self) {
        let sessions = self.sessions.lock().unwrap();
        if let Ok(data) = serde_json::to_string_pretty(&*sessions) {
            let _ = fs::write(self.data_dir.join("sessions.json"), data);
        }
    }

    fn default_skills() -> Vec<Skill> {
        vec![
            Skill {
                id: "debugger".into(),
                name: "debugger".into(),
                category: "Coding".into(),
                description: "Debugging skill".into(),
                instructions: "1. Reproduce 2. Isolate 3. Fix 4. Verify".into(),
                enabled: true,
            },
            Skill {
                id: "developer".into(),
                name: "developer".into(),
                category: "Coding".into(),
                description: "Development skill".into(),
                instructions: "Clean code principles".into(),
                enabled: true,
            },
            Skill {
                id: "qa".into(),
                name: "qa".into(),
                category: "Testing".into(),
                description: "QA skill".into(),
                instructions: "Test pyramid".into(),
                enabled: true,
            },
            Skill {
                id: "pentester".into(),
                name: "pentester".into(),
                category: "Security".into(),
                description: "Security testing".into(),
                instructions: "OWASP methodology".into(),
                enabled: true,
            },
            Skill {
                id: "blueteamer".into(),
                name: "blueteamer".into(),
                category: "Security".into(),
                description: "Defensive security".into(),
                instructions: "Defense in depth".into(),
                enabled: true,
            },
            Skill {
                id: "architect".into(),
                name: "architect".into(),
                category: "Coding".into(),
                description: "Architecture skill".into(),
                instructions: "System design principles".into(),
                enabled: true,
            },
            Skill {
                id: "devops".into(),
                name: "devops".into(),
                category: "DevOps".into(),
                description: "DevOps skill".into(),
                instructions: "CI/CD pipeline".into(),
                enabled: true,
            },
            Skill {
                id: "data-engineer".into(),
                name: "data-engineer".into(),
                category: "Data".into(),
                description: "Data engineering".into(),
                instructions: "Data pipeline design".into(),
                enabled: true,
            },
            Skill {
                id: "api-design".into(),
                name: "api-design".into(),
                category: "Coding".into(),
                description: "API design and REST/GraphQL".into(),
                instructions:
                    "REST: Resources, Verbs, Status codes. GraphQL: Types, Resolvers, Queries"
                        .into(),
                enabled: true,
            },
            Skill {
                id: "document-reviewer".into(),
                name: "document-reviewer".into(),
                category: "Docs".into(),
                description: "Technical documentation review".into(),
                instructions: "Clarity, Completeness, Accuracy, Examples, Accessibility".into(),
                enabled: true,
            },
            Skill {
                id: "ux-design".into(),
                name: "ux-design".into(),
                category: "Design".into(),
                description: "User experience design".into(),
                instructions: "User research, Wireframes, Prototypes, Usability testing".into(),
                enabled: true,
            },
            Skill {
                id: "sre".into(),
                name: "sre".into(),
                category: "DevOps".into(),
                description: "Site Reliability Engineering".into(),
                instructions: "SLIs/SLOs, Alerting, On-call, Postmortems, Automation".into(),
                enabled: true,
            },
            Skill {
                id: "product-manager".into(),
                name: "product-manager".into(),
                category: "Product".into(),
                description: "Product management and prioritization".into(),
                instructions:
                    "User stories, Prioritization (MoSCoW), Roadmap planning, Stakeholder mgmt"
                        .into(),
                enabled: true,
            },
        ]
    }

    fn register_session(&self, agent_type: &str, instance: &str, session_id: &str, project: &str, terminal_path: Option<String>) {
        let mut sessions = self.sessions.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        sessions.retain(|s| s.agent_type != agent_type || s.project != project);
        sessions.push(AgentSession {
            agent_type: agent_type.into(),
            instance: instance.into(),
            session_id: session_id.into(),
            project: project.into(),
            last_seen: now,
            auto_reconnect: true,
            terminal_path,
        });
        drop(sessions);
        self.save_sessions();
    }

    fn find_session(&self, agent_type: &str, project: &str) -> Option<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .iter()
            .find(|s| s.agent_type == agent_type && s.project == project && s.auto_reconnect)
            .map(|s| s.session_id.clone())
    }

    fn publish_event(&self, event: Event) {
        self.event_bus.publish(event);
    }

    fn _poll_events(&self, since_timestamp: i64) -> Vec<Event> {
        self.event_bus.poll(since_timestamp)
    }

    fn register_connection(&self, session_id: &str, agent_type: &str, project: &str) {
        let mut conns = self.active_connections.lock().unwrap();
        conns.retain(|c| c.session_id != session_id);
        conns.push(ConnectionInfo {
            session_id: session_id.to_string(),
            agent_type: agent_type.to_string(),
            project: project.to_string(),
            last_activity: Instant::now(),
        });
        drop(conns);
        let mut event = Event::new("agent_connected");
        event.session_id = Some(session_id.to_string());
        event.agent_type = Some(agent_type.to_string());
        event.project = Some(project.to_string());
        self.publish_event(event);
    }

    fn _unregister_connection(&self, session_id: &str) {
        let mut conns = self.active_connections.lock().unwrap();
        conns.retain(|c| c.session_id != session_id);
        drop(conns);
        let mut event = Event::new("agent_disconnected");
        event.session_id = Some(session_id.to_string());
        self.publish_event(event);
    }

    fn _get_active_agents_info(&self) -> Vec<serde_json::Value> {
        let conns = self.active_connections.lock().unwrap();
        conns
            .iter()
            .map(|c| {
                serde_json::json!({
                    "session_id": c.session_id,
                    "agent_type": c.agent_type,
                    "project": c.project,
                    "seconds_since_activity": c.last_activity.elapsed().as_secs()
                })
            })
            .collect()
    }

    fn _send_message(&self, from: &str, to: Option<&str>, content: &str) {
        // Publish event for general messaging
        let mut event = Event::new("message");
        event.from = Some(from.to_string());
        event.to = to.map(String::from);
        event.content = Some(content.to_string());
        self.publish_event(event);

        // If a specific recipient is specified, try to send to their terminal
        if let Some(to_session) = to {
            let sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.iter().find(|s| s.session_id == to_session) {
                if let Some(ref terminal_path) = session.terminal_path {
                    // Attempt to send to terminal, but don't fail if it doesn't work
                    let _ = self.send_to_terminal(terminal_path, content);
                }
            }
        }
    }

    fn _broadcast(&self, from: &str, content: &str) {
        let mut event = Event::new("broadcast");
        event.from = Some(from.to_string());
        event.content = Some(content.to_string());
        self.publish_event(event);
    }

    fn send_to_terminal(&self, terminal_path: &str, content: &str) -> Result<(), std::io::Error> {
        use std::fs::OpenOptions;
        use std::io::Write;
        let mut file = OpenOptions::new().write(true).open(terminal_path)?;
        file.write_all(content.as_bytes())?;
        file.write_all(b"\n")?; // Simulate Enter key
        Ok(())
    }

    fn get_agent_default_skills(&self, agent_type: &str) -> Vec<String> {
        match agent_type {
            "opencode" | "qwen" | "qwen-code" => {
                vec!["developer".into(), "debugger".into(), "architect".into()]
            }
            "claude" => {
                vec!["developer".into(), "qa".into(), "architect".into()]
            }
            "gemini" => {
                vec!["developer".into(), "data-engineer".into()]
            }
            "cursor" | "windsurf" => {
                vec!["developer".into(), "debugger".into()]
            }
            "copilot" => {
                vec!["developer".into()]
            }
            _ => vec!["developer".into()],
        }
    }
}

fn dirs_data_local_dir() -> Option<PathBuf> {
    std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".local/share"))
        })
}

fn _handle_tcp_client(
    stream: TcpStream,
    state: Arc<ServerState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream.try_clone()?;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("GET /events") {
            let _session_id = line
                .lines()
                .find(|l| l.starts_with("X-Session-ID:"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim())
                .unwrap_or("anonymous")
                .to_string();

            writeln!(writer, "HTTP/1.1 200 OK")?;
            writeln!(writer, "Content-Type: text/event-stream")?;
            writeln!(writer, "Cache-Control: no-cache")?;
            writeln!(writer, "Connection: keep-alive")?;
            writeln!(writer, "Access-Control-Allow-Origin: *")?;
            writeln!(writer)?;
            writer.flush()?;

            let start_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            loop {
                let events = state._poll_events(start_time);
                for event in events {
                    let event_json = serde_json::to_string(&event).unwrap_or_default();
                    writeln!(writer, "data: {}", event_json)?;
                    writer.flush()?;
                }
                thread::sleep(std::time::Duration::from_secs(1));

                let mut buf = [0u8; 1];
                if stream.peek(&mut buf).is_ok() {
                    break;
                }
            }
            continue;
        }

        let response = handle_request(line, &state);
        writeln!(writer, "{}", response)?;
        writer.flush()?;
    }
    Ok(())
}

fn handle_request(line: &str, state: &Arc<ServerState>) -> String {
    let json: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return r#"{"error":"Invalid JSON","id":null}"#.to_string(),
    };

    let id = json.get("id");
    let method = json.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = json.get("params");

    let result = match method {
        "ping" => serde_json::json!({"status": "ok"}),

        "session_register" => {
            let args = params.and_then(|p| p.get("arguments"));
            let agent_type = args
                .and_then(|a| a.get("agent_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let project = args
                .and_then(|a| a.get("project"))
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let terminal_path = args
                .and_then(|a| a.get("terminal_path"))
                .and_then(|v| v.as_str())
                .map(String::from);

            // Whitelist: only allow identified AI agents
            let allowed = [
                "opencode",
                "qwen",
                "qwen-code",
                "claude",
                "gemini",
                "cursor",
                "windsurf",
                "copilot",
            ];
            if !allowed
                .iter()
                .any(|&a| agent_type == a || agent_type.starts_with(a))
            {
                return serde_json::json!({
                    "error": format!("Unauthorized agent: {}. Only identified AI agents allowed.", agent_type)
                }).to_string();
            }

            // Auto-reconnect: find existing session for this agent_type
            let (session_id, reconnected) =
                if let Some(existing) = state.find_session(agent_type, project) {
                    (existing, true)
                } else {
                    let instance = Uuid::new_v4().to_hex_string();
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    (format!("{}-{}-{}", agent_type, instance, ts), false)
                };

            let instance_id = session_id.split('-').nth(1).unwrap_or("x");
            if let Err(e) =
                state
                    .shared
                    .db
                    .register_agent_session(agent_type, instance_id, project, None)
            {
                serde_json::json!({"error": format!("{:?}", e)})
            } else {
                let _ = state.shared.db.agent_heartbeat(
                    &session_id,
                    Some(if reconnected {
                        "reconnected"
                    } else {
                        "registered"
                    }),
                );
                state.register_session(agent_type, &session_id, &session_id, project, terminal_path);
                state.register_connection(&session_id, agent_type, project);

                let agent_skills = state.get_agent_default_skills(agent_type);
                state
                    .orchestrator
                    .register_agent_with_id(&session_id, agent_type, agent_skills);

                // PROACTIVE: When agent registers, immediately check for pending tasks
                if let Some(_task) = state.orchestrator.proactive_assign_to(&session_id) {
                    state.orchestrator.save(&state.data_dir);
                }

                // Get pending messages for this session
                let pending = state.event_bus.get_pending_messages(&session_id);
                serde_json::json!({
                    "session_id": session_id,
                    "reconnected": reconnected,
                    "pending_messages": pending
                })
            }
        }

        "session_reconnect" => {
            let args = params.and_then(|p| p.get("arguments"));
            let agent_type = args
                .and_then(|a| a.get("agent_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let project = args
                .and_then(|a| a.get("project"))
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let terminal_path = args
                .and_then(|a| a.get("terminal_path"))
                .and_then(|v| v.as_str())
                .map(String::from);

            if let Some(old_session) = state.find_session(agent_type, project) {
                let _ = state
                    .shared
                    .db
                    .agent_heartbeat(&old_session, Some("reconnected"));
                serde_json::json!({"session_id": old_session, "reconnected": true})
            } else {
                let instance = Uuid::new_v4().to_hex_string();
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let session_id = format!("{}-{}-{}", agent_type, instance, ts);
                let _ = state
                    .shared
                    .db
                    .register_agent_session(agent_type, &instance, project, None);
                let _ = state
                    .shared
                    .db
                    .agent_heartbeat(&session_id, Some("new-session"));
                state.register_session(agent_type, &instance, &session_id, project, terminal_path);
                serde_json::json!({"session_id": session_id, "reconnected": false})
            }
        }

        "agent_heartbeat" => {
            let args = params.and_then(|p| p.get("arguments"));
            let session_id = args
                .and_then(|a| a.get("session_id"))
                .and_then(|v| v.as_str());
            let task = args.and_then(|a| a.get("task")).and_then(|v| v.as_str());
            let status = args
                .and_then(|a| a.get("status"))
                .and_then(|v| v.as_str())
                .and_then(|s| match s {
                    "idle" => Some(AgentStatus::Idle),
                    "busy" => Some(AgentStatus::Busy),
                    "thinking" => Some(AgentStatus::Thinking),
                    "waiting" => Some(AgentStatus::Waiting),
                    _ => None,
                });
            match session_id {
                Some(sid) => {
                    state.orchestrator.heartbeat(sid, status, task);
                    if let Err(e) = state.shared.db.agent_heartbeat(sid, task) {
                        serde_json::json!({"error": format!("{:?}", e)})
                    } else {
                        if let Some(notification) =
                            state.orchestrator.get_agent_task_notification(sid)
                        {
                            serde_json::json!({
                                "status": "ok",
                                "task_assigned": true,
                                "task": notification
                            })
                        } else {
                            serde_json::json!({"status": "ok"})
                        }
                    }
                }
                None => serde_json::json!({"error": "session_id required"}),
            }
        }

        "send_message" => {
            let args = params.and_then(|p| p.get("arguments"));
            let from_session = args
                .and_then(|a| a.get("session_id"))
                .and_then(|v| v.as_str());
            let to_session = args
                .and_then(|a| a.get("to"))
                .and_then(|v| v.as_str());
            let content = args
                .and_then(|a| a.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match (from_session, to_session) {
                (Some(from), Some(to)) => {
                    state._send_message(from, Some(to), content);
                    serde_json::json!({"status": "sent"})
                }
                _ => serde_json::json!({"error": "session_id and to are required"}),
            }
        }

        "agents_active" => {
            let project = params
                .and_then(|p| p.get("project"))
                .and_then(|v| v.as_str());
            match state.shared.db.get_active_agents(project) {
                Ok(agents) => serde_json::json!({"agents": agents}),
                Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
            }
        }

        "skill_list" => {
            let skills = state.skills.lock().unwrap();
            serde_json::json!({"skills": &*skills})
        }

        "skill_activate" => {
            let args = params.and_then(|p| p.get("arguments"));
            let skill_id = args
                .and_then(|a| a.get("skill_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({"activated": skill_id, "status": "ok"})
        }

        "skill_register" => {
            let args = params.and_then(|p| p.get("arguments"));
            let name = args
                .and_then(|a| a.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let category = args
                .and_then(|a| a.get("category"))
                .and_then(|v| v.as_str())
                .unwrap_or("Custom")
                .to_string();
            let description = args
                .and_then(|a| a.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if name.is_empty() {
                serde_json::json!({"error": "name is required"})
            } else {
                let skill = Skill {
                    id: Uuid::new_v4().to_hex_string(),
                    name: name.clone(),
                    category,
                    description,
                    instructions: String::new(),
                    enabled: true,
                };
                state.skills.lock().unwrap().push(skill.clone());
                serde_json::json!({"skill_id": skill.id, "name": skill.name, "status": "registered"})
            }
        }

        "task_create" => {
            let args = params.and_then(|p| p.get("arguments"));
            let _project = args
                .and_then(|a| a.get("project"))
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let _task_type = args
                .and_then(|a| a.get("task_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("general");
            let payload = args
                .and_then(|a| a.get("payload"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let priority = args
                .and_then(|a| a.get("priority"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u8;
            let skills: Vec<String> = args
                .and_then(|a| a.get("required_skills"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_else(|| vec!["developer".into()]);

            let task_id = state
                .orchestrator
                .create_task(payload, skills.clone(), priority, None);

            if let Some(agent_id) = state.orchestrator.find_best_agent(&skills) {
                state.orchestrator.assign_task(&task_id, &agent_id);
                state.orchestrator.save(&state.data_dir);

                state.orchestrator.log_message(
                    "orchestrator",
                    Some(&agent_id),
                    synapsis::core::orchestrator::MessageType::TaskResponse,
                    serde_json::json!({
                        "action": "task_assigned",
                        "task_id": task_id,
                        "description": payload,
                        "priority": priority,
                        "required_skills": skills
                    }),
                );

                serde_json::json!({
                    "task_id": task_id,
                    "assigned_to": agent_id,
                    "auto_assigned": true
                })
            } else {
                serde_json::json!({
                    "task_id": task_id,
                    "assigned_to": null,
                    "auto_assigned": false,
                    "pending_agent": true
                })
            }
        }

        "task_claim" => {
            let args = params.and_then(|p| p.get("arguments"));
            let session_id = args
                .and_then(|a| a.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            match state.shared.db.claim_task(session_id, None) {
                Ok(task) => serde_json::json!({"task": task}),
                Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
            }
        }

        "task_request" => {
            let args = params.and_then(|p| p.get("arguments"));
            let session_id = args
                .and_then(|a| a.get("session_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let preferred_skills: Vec<String> = args
                .and_then(|a| a.get("skills"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            state
                .orchestrator
                .heartbeat(session_id, Some(AgentStatus::Idle), None);

            if let Some(pending) = state.orchestrator.get_pending_tasks().first() {
                if preferred_skills.is_empty()
                    || preferred_skills
                        .iter()
                        .any(|s| pending.required_skills.contains(s))
                {
                    if state.orchestrator.assign_task(&pending.id, session_id) {
                        serde_json::json!({
                            "task_id": pending.id,
                            "description": pending.description,
                            "required_skills": pending.required_skills,
                            "assigned": true
                        })
                    } else {
                        serde_json::json!({
                            "task_id": null,
                            "assigned": false,
                            "message": "Failed to assign task"
                        })
                    }
                } else {
                    serde_json::json!({
                        "task_id": null,
                        "assigned": false,
                        "message": "No matching tasks for preferred skills"
                    })
                }
            } else {
                serde_json::json!({
                    "task_id": null,
                    "assigned": false,
                    "message": "No pending tasks available"
                })
            }
        }

        "task_complete" => {
            let args = params.and_then(|p| p.get("arguments"));
            let task_id = args
                .and_then(|a| a.get("task_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let success = args
                .and_then(|a| a.get("success"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            state.orchestrator.complete_task(task_id, success);

            serde_json::json!({"completed": true, "task_id": task_id})
        }

        "task_delegate" => {
            let args = params.and_then(|p| p.get("arguments"));
            let task_id = args
                .and_then(|a| a.get("task_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let from_agent = args
                .and_then(|a| a.get("from"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if let Some(delegated_to) = state.orchestrator.delegate_task(task_id, from_agent) {
                serde_json::json!({
                    "delegated": true,
                    "task_id": task_id,
                    "to_agent": delegated_to
                })
            } else {
                serde_json::json!({
                    "delegated": false,
                    "task_id": task_id,
                    "reason": "No suitable agent found"
                })
            }
        }

        _ => {
            serde_json::json!({
                "error": format!("Unknown method: {}", method),
                "code": -32601
            })
        }
    };

    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "result": result,
        "id": id
    });

    serde_json::to_string(&response).unwrap_or_else(|e| {
        format!("{{\"jsonrpc\":\"2.0\",\"error\":{{\"message\":\"{}\"}},\"id\":null}}", e)
    })
}

fn main() {
    println!("Synapsis v{} - YOLO Mode Active", env!("CARGO_PKG_VERSION"));
    println!("TCP Server: 127.0.0.1:7438");
    println!("Ready for autonomous multi-agent operations");
    
    let state = Arc::new(ServerState::new());
    
    let addr = format!("127.0.0.1:{}", 7438);
    let listener = std::net::TcpListener::bind(&addr).unwrap();
    println!("[TCP] Listening on {}", addr);
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                let peer_addr = match stream.peer_addr() {
                    Ok(addr) => addr.to_string(),
                    Err(_) => "unknown".to_string(),
                };
                std::thread::spawn(move || {
                    let cloned_stream = match stream.try_clone() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[TCP] Failed to clone stream: {}", e);
                            return;
                        }
                    };
                    let mut reader = std::io::BufReader::new(cloned_stream);
                    let mut writer = stream;
                    let mut line = String::new();
                    loop {
                        match reader.read_line(&mut line) {
                            Ok(0) => break, // EOF
                            Ok(_) => {
                                if line.trim().is_empty() {
                                    line.clear();
                                    continue;
                                }
                                // Rate limiting per connection
                                if let Err(e) = state.rate_limiter.check(&peer_addr) {
                                    let error_response = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "error": {"code": -32000, "message": format!("Rate limit exceeded: {}", e)},
                                        "id": null,
                                    });
                                    if let Ok(json) = serde_json::to_string(&error_response) {
                                        let _ = writeln!(writer, "{}", json);
                                        let _ = writer.flush();
                                    }
                                    line.clear();
                                    continue;
                                }
                                let response = handle_request(&line, &state);
                                let _ = writeln!(writer, "{}", response);
                                let _ = writer.flush();
                                line.clear();
                            }
                            Err(e) => {
                                eprintln!("[TCP] Read error: {}", e);
                                break;
                            }
                        }
                    }
                });
            }
            Err(e) => eprintln!("[TCP] Connection error: {}", e),
        }
    }
}
