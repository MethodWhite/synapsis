pub mod cleanup;
pub mod code;
pub mod connectors;
pub mod file;
pub mod git;
pub mod search;
pub mod shell;

use crate::core::lock_utils::*;
pub use code::CodeWorker;
pub use connectors::{OpenCodeConnector, QwenConnector};
pub use file::FileWorker;
pub use git::GitWorker;
pub use search::SearchWorker;
pub use shell::ShellWorker;

use crate::core::uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub trait WorkerAgent: Send + Sync {
    fn id(&self) -> &str;
    fn skills(&self) -> Vec<String>;
    fn capacity(&self) -> usize;
    fn execute_task(&self, task: &Task) -> Result<TaskResult, TaskError>;
    fn heartbeat(&self) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub payload: serde_json::Value,
    pub deadline: Option<i64>,
    #[serde(rename = "skills_required")]
    pub skills_required: Vec<String>,
}

impl Task {
    pub fn new(task_type: TaskType, payload: serde_json::Value) -> Self {
        Self {
            id: format!("task-{}", Uuid::new_v4().to_hex_string()),
            task_type,
            payload,
            deadline: None,
            skills_required: Vec::new(),
        }
    }

    pub fn with_deadline(mut self, deadline: i64) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn with_skills(mut self, skills: Vec<String>) -> Self {
        self.skills_required = skills;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Shell,
    FileRead,
    FileWrite,
    CodeAnalysis,
    CodeRefactor,
    Search,
    Git,
    Delegate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub output: String,
    pub status: TaskStatus,
    pub duration_ms: u64,
    pub logs: Vec<String>,
}

impl TaskResult {
    pub fn success(output: String, duration_ms: u64) -> Self {
        Self {
            output,
            status: TaskStatus::Success,
            duration_ms,
            logs: Vec::new(),
        }
    }

    pub fn failure(msg: String, duration_ms: u64) -> Self {
        Self {
            output: msg,
            status: TaskStatus::Failed,
            duration_ms,
            logs: Vec::new(),
        }
    }

    pub fn with_logs(mut self, logs: Vec<String>) -> Self {
        self.logs = logs;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Success,
    Failed,
    Timeout,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskError {
    pub code: u16,
    pub message: String,
    pub task_id: String,
}

impl TaskError {
    pub fn new(code: u16, message: &str, task_id: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            task_id: task_id.to_string(),
        }
    }

    pub fn unsupported_operation(op: &str, task_id: &str) -> Self {
        Self::new(0x0001, &format!("Unsupported operation: {}", op), task_id)
    }

    pub fn execution_failed(msg: &str, task_id: &str) -> Self {
        Self::new(0x0002, msg, task_id)
    }

    pub fn not_found(what: &str, task_id: &str) -> Self {
        Self::new(0x0003, &format!("{} not found", what), task_id)
    }

    pub fn permission_denied(path: &str, task_id: &str) -> Self {
        Self::new(0x0004, &format!("Permission denied: {}", path), task_id)
    }

    pub fn timeout(task_id: &str) -> Self {
        Self::new(0x0005, "Task execution timed out", task_id)
    }
}

pub struct WorkerRegistry {
    workers: Arc<Mutex<HashMap<String, Arc<dyn WorkerAgent>>>>,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, worker: Arc<dyn WorkerAgent>) {
        let id = worker.id().to_string();
        self.workers.lock_safe().insert(id, worker);
    }

    pub fn unregister(&self, id: &str) {
        self.workers.lock_safe().remove(id);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn WorkerAgent>> {
        self.workers.lock_safe().get(id).cloned()
    }

    pub fn find_by_skill(&self, skill: &str) -> Vec<Arc<dyn WorkerAgent>> {
        self.workers
            .lock_safe()
            .values()
            .filter(|w| w.skills().contains(&skill.to_string()))
            .cloned()
            .collect()
    }

    pub fn list(&self) -> Vec<String> {
        self.workers.lock_safe().keys().cloned().collect()
    }
}

impl Default for WorkerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WorkerOrchestrator {
    registry: WorkerRegistry,
    external_connectors: HashMap<String, Box<dyn connectors::ExternalAgentConnector>>,
}

impl WorkerOrchestrator {
    pub fn new() -> Self {
        Self {
            registry: WorkerRegistry::new(),
            external_connectors: HashMap::new(),
        }
    }

    pub fn register_worker(&mut self, worker: Arc<dyn WorkerAgent>) {
        self.registry.register(worker);
    }

    pub fn register_connector(&mut self, connector: Box<dyn connectors::ExternalAgentConnector>) {
        let id = connector.agent_id().to_string();
        self.external_connectors.insert(id, connector);
    }

    pub fn find_worker_for_task(&self, task: &Task) -> Option<Arc<dyn WorkerAgent>> {
        for skill in &task.skills_required {
            let workers = self.registry.find_by_skill(skill);
            if let Some(worker) = workers.first() {
                return Some(worker.clone());
            }
        }
        None
    }

    pub fn execute_task(&self, task: Task) -> Result<TaskResult, TaskError> {
        if let Some(worker) = self.find_worker_for_task(&task) {
            worker.execute_task(&task)
        } else {
            Err(TaskError::unsupported_operation(
                "No suitable worker found",
                &task.id,
            ))
        }
    }

    pub fn get_status(&self) -> serde_json::Value {
        serde_json::json!({
            "builtin_workers": self.registry.list(),
            "external_agents": self.external_connectors.keys().collect::<Vec<_>>(),
        })
    }
}

impl Default for WorkerOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AgentDiscovery {
    registry: WorkerRegistry,
}

impl AgentDiscovery {
    pub fn new() -> Self {
        Self {
            registry: WorkerRegistry::new(),
        }
    }

    pub async fn discover_available_agents(&self) -> Vec<AvailableAgent> {
        let mut agents = Vec::new();

        if let Ok(output) = std::process::Command::new("which").arg("opencode").output() {
            if output.status.success() {
                agents.push(AvailableAgent {
                    name: "opencode".to_string(),
                    path: "opencode".to_string(),
                    connector_type: "opencode".to_string(),
                });
            }
        }

        if let Ok(output) = std::process::Command::new("which").arg("qwen").output() {
            if output.status.success() {
                agents.push(AvailableAgent {
                    name: "qwen".to_string(),
                    path: "qwen".to_string(),
                    connector_type: "qwen".to_string(),
                });
            }
        }

        if let Ok(output) = std::process::Command::new("which").arg("claude").output() {
            if output.status.success() {
                agents.push(AvailableAgent {
                    name: "claude".to_string(),
                    path: "claude".to_string(),
                    connector_type: "claude".to_string(),
                });
            }
        }

        agents
    }

    pub fn register_builtin_workers(&self) {
        self.registry.register(Arc::new(shell::ShellWorker::new()));
        self.registry.register(Arc::new(file::FileWorker::new()));
        self.registry.register(Arc::new(code::CodeWorker::new()));
        self.registry
            .register(Arc::new(search::SearchWorker::new()));
        self.registry.register(Arc::new(git::GitWorker::new()));
    }

    pub fn get_registry(&self) -> &WorkerRegistry {
        &self.registry
    }
}

impl Default for AgentDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableAgent {
    pub name: String,
    pub path: String,
    pub connector_type: String,
}
