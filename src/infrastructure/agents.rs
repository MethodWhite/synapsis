//! Synapsis Agents Module
//!
//! Multi-agent coordination system. Agents are AI assistants with specialized
//! roles and capabilities. Synapsis manages agent definitions, states,
//! communication, and coordination.

use crate::core::uuid::Uuid;
use crate::domain::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub role: AgentRole,
    pub description: String,
    pub capabilities: Vec<Capability>,
    pub skills: Vec<String>,
    pub model: Option<String>,
    pub state: AgentState,
    pub memory_scope: MemoryScope,
    pub created_at: Timestamp,
    pub last_active: Timestamp,
    pub metadata: HashMap<String, String>,
}

impl Agent {
    pub fn new(name: String, role: AgentRole, description: String) -> Self {
        let now = Timestamp::now();
        Self {
            id: AgentId::new(),
            name,
            role,
            description,
            capabilities: Vec::new(),
            skills: Vec::new(),
            model: None,
            state: AgentState::Idle,
            memory_scope: MemoryScope::Project,
            created_at: now,
            last_active: now,
            metadata: HashMap::new(),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    pub fn with_skills(mut self, skills: Vec<String>) -> Self {
        self.skills = skills;
        self
    }

    pub fn update_activity(&mut self) {
        self.last_active = Timestamp::now();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum AgentRole {
    Orchestrator = 0,
    Coder = 1,
    Researcher = 2,
    Reviewer = 3,
    Tester = 4,
    Architect = 5,
    Security = 6,
    DevOps = 7,
    General = 8,
}

impl AgentRole {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "orchestrator" | "orch" | "coordinator" => Self::Orchestrator,
            "coder" | "code" | "developer" => Self::Coder,
            "researcher" | "research" => Self::Researcher,
            "reviewer" | "review" | "critic" => Self::Reviewer,
            "tester" | "test" | "qa" => Self::Tester,
            "architect" | "arch" => Self::Architect,
            "security" | "sec" => Self::Security,
            "devops" | "ops" => Self::DevOps,
            _ => Self::General,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum AgentState {
    Idle = 0,
    Working = 1,
    Waiting = 2,
    Paused = 3,
    Error = 4,
    Terminated = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MemoryScope {
    Project = 0,
    Personal = 1,
    Shared = 2,
    Global = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub weight: f32,
}

impl Capability {
    pub fn new(name: &str, description: &str, weight: f32) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            weight: weight.clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: MessageId,
    pub from: AgentId,
    pub to: AgentId,
    pub content: String,
    pub message_type: MessageType,
    pub created_at: Timestamp,
    pub delivered: bool,
}

impl AgentMessage {
    pub fn new(from: AgentId, to: AgentId, content: String, msg_type: MessageType) -> Self {
        Self {
            id: MessageId::new(),
            from,
            to,
            content,
            message_type: msg_type,
            created_at: Timestamp::now(),
            delivered: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessageType {
    Task = 0,
    Response = 1,
    Status = 2,
    Error = 3,
    Coordination = 4,
    Broadcast = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub description: String,
    pub assigned_to: Option<AgentId>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub completed_at: Option<Timestamp>,
    pub result: Option<String>,
}

impl Task {
    pub fn new(title: String, description: String, priority: TaskPriority) -> Self {
        let now = Timestamp::now();
        Self {
            id: TaskId::new(),
            title,
            description,
            assigned_to: None,
            status: TaskStatus::Pending,
            priority,
            created_at: now,
            updated_at: now,
            completed_at: None,
            result: None,
        }
    }

    pub fn assign(&mut self, agent_id: AgentId) {
        self.assigned_to = Some(agent_id);
        self.status = TaskStatus::Assigned;
        self.updated_at = Timestamp::now();
    }

    pub fn complete(&mut self, result: String) {
        self.result = Some(result);
        self.status = TaskStatus::Completed;
        self.completed_at = Some(Timestamp::now());
        self.updated_at = Timestamp::now();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TaskStatus {
    Pending = 0,
    Assigned = 1,
    InProgress = 2,
    Completed = 3,
    Failed = 4,
    Cancelled = 5,
}

impl TaskStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "assigned" => Some(Self::Assigned),
            "inprogress" | "in_progress" | "progress" => Some(Self::InProgress),
            "completed" | "done" => Some(Self::Completed),
            "failed" | "error" => Some(Self::Failed),
            "cancelled" | "canceled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_hex_string())
    }
    pub fn from_string(s: String) -> Self {
        Self(s)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_hex_string())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct MessageId(pub String);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_hex_string())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<AgentId, Agent>>>,
    messages: Arc<RwLock<Vec<AgentMessage>>>,
    tasks: Arc<RwLock<Vec<Task>>>,
    data_dir: PathBuf,
}

impl AgentRegistry {
    pub fn new() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapsis")
            .join("agents");
        std::fs::create_dir_all(&data_dir).ok();

        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(Vec::new())),
            tasks: Arc::new(RwLock::new(Vec::new())),
            data_dir,
        }
    }

    pub fn init(&self) -> std::io::Result<()> {
        self.load()
    }

    pub fn load(&self) -> std::io::Result<()> {
        let agents_file = self.data_dir.join("agents.json");
        if agents_file.exists() {
            if let Ok(data) = std::fs::read_to_string(&agents_file) {
                if let Ok(agents) = serde_json::from_str::<HashMap<AgentId, Agent>>(&data) {
                    *self.agents.write().unwrap() = agents;
                }
            }
        }

        let tasks_file = self.data_dir.join("tasks.json");
        if tasks_file.exists() {
            if let Ok(data) = std::fs::read_to_string(&tasks_file) {
                if let Ok(tasks) = serde_json::from_str::<Vec<Task>>(&data) {
                    *self.tasks.write().unwrap() = tasks;
                }
            }
        }

        Ok(())
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;

        let agents_file = self.data_dir.join("agents.json");
        let agents = self.agents.read().unwrap();
        let data = serde_json::to_string_pretty(&*agents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(agents_file, data)?;

        let tasks_file = self.data_dir.join("tasks.json");
        let tasks = self.tasks.read().unwrap();
        let data = serde_json::to_string_pretty(&*tasks)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(tasks_file, data)?;

        Ok(())
    }

    pub fn register(&self, agent: Agent) -> AgentId {
        let id = agent.id.clone();
        self.agents.write().unwrap().insert(id.clone(), agent);
        let _ = self.save();
        id
    }

    pub fn unregister(&self, id: &AgentId) -> Option<Agent> {
        let agent = self.agents.write().unwrap().remove(id);
        let _ = self.save();
        agent
    }

    pub fn get(&self, id: &AgentId) -> Option<Agent> {
        self.agents.read().unwrap().get(id).cloned()
    }

    pub fn get_by_name(&self, name: &str) -> Option<Agent> {
        self.agents
            .read()
            .unwrap()
            .values()
            .find(|a| a.name == name)
            .cloned()
    }

    pub fn list(&self, role: Option<AgentRole>) -> Vec<Agent> {
        let agents = self.agents.read().unwrap();
        match role {
            Some(r) => agents.values().filter(|a| a.role == r).cloned().collect(),
            None => agents.values().cloned().collect(),
        }
    }

    pub fn update_state(&self, id: &AgentId, state: AgentState) -> bool {
        if let Some(agent) = self.agents.write().unwrap().get_mut(id) {
            agent.state = state;
            if state == AgentState::Working {
                agent.update_activity();
            }
            let _ = self.save();
            true
        } else {
            false
        }
    }

    pub fn send_message(
        &self,
        from: AgentId,
        to: AgentId,
        content: String,
        msg_type: MessageType,
    ) -> Option<MessageId> {
        if !self.agents.read().unwrap().contains_key(&from)
            || !self.agents.read().unwrap().contains_key(&to)
        {
            return None;
        }

        let msg = AgentMessage::new(from, to, content, msg_type);
        let id = msg.id.clone();
        self.messages.write().unwrap().push(msg);
        let _ = self.save();
        Some(id)
    }

    pub fn get_messages(&self, agent_id: &AgentId, limit: usize) -> Vec<AgentMessage> {
        self.messages
            .read()
            .unwrap()
            .iter()
            .filter(|m| m.to == *agent_id || m.from == *agent_id)
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn create_task(
        &self,
        title: String,
        description: String,
        priority: TaskPriority,
    ) -> TaskId {
        let task = Task::new(title, description, priority);
        let id = task.id.clone();
        self.tasks.write().unwrap().push(task);
        let _ = self.save();
        id
    }

    pub fn assign_task(&self, task_id: &TaskId, agent_id: &AgentId) -> bool {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == *task_id) {
            task.assign(agent_id.clone());
            drop(tasks);
            let _ = self.save();
            true
        } else {
            false
        }
    }

    pub fn complete_task(&self, task_id: &TaskId, result: String) -> bool {
        let mut tasks = self.tasks.write().unwrap();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == *task_id) {
            task.complete(result);
            drop(tasks);
            let _ = self.save();
            true
        } else {
            false
        }
    }

    pub fn get_tasks(&self, status: Option<TaskStatus>) -> Vec<Task> {
        let tasks = self.tasks.read().unwrap();
        match status {
            Some(s) => tasks.iter().filter(|t| t.status == s).cloned().collect(),
            None => tasks.iter().cloned().collect(),
        }
    }

    pub fn count(&self) -> usize {
        self.agents.read().unwrap().len()
    }

    pub fn get_active_count(&self) -> usize {
        self.agents
            .read()
            .unwrap()
            .values()
            .filter(|a| a.state == AgentState::Working)
            .count()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AgentRegistry {
    fn clone(&self) -> Self {
        Self {
            agents: self.agents.clone(),
            messages: self.messages.clone(),
            tasks: self.tasks.clone(),
            data_dir: self.data_dir.clone(),
        }
    }
}

mod dirs {
    use std::path::PathBuf;
    pub fn data_local_dir() -> Option<PathBuf> {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local/share"))
            })
            .or_else(|| std::env::var("APPDATA").ok().map(PathBuf::from))
    }
}
