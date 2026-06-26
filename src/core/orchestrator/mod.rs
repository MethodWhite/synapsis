mod agent;
mod communication;
mod legacy;
mod task;
pub mod types;

use crate::core::lock_utils::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::core::resource_manager::{AgentLimits, ResourceManager};

pub use types::{
    Agent, AgentStatus, LegacyFile, MessageType, OrchestratorMessage, ReviewStatus, Task,
    TaskStatus,
};

pub struct Orchestrator {
    pub(crate) agents: Arc<Mutex<HashMap<String, Agent>>>,
    pub(crate) tasks: Arc<Mutex<HashMap<String, Task>>>,
    pub(crate) messages: Arc<Mutex<Vec<OrchestratorMessage>>>,
    pub(crate) skills_index: Arc<Mutex<HashMap<String, Vec<String>>>>,
    pub(crate) resource_manager: Arc<ResourceManager>,
    pub(crate) legacy_files: Arc<Mutex<HashMap<String, LegacyFile>>>,
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl Orchestrator {
    pub fn new() -> Self {
        let resource_manager = Arc::new(ResourceManager::new());

        resource_manager.set_agent_limits(
            "opencode",
            AgentLimits {
                max_concurrent_tasks: 3,
                max_cpu_percent: 50.0,
                max_memory_mb: 2048,
                priority: 8,
            },
        );
        resource_manager.set_agent_limits(
            "qwen",
            AgentLimits {
                max_concurrent_tasks: 2,
                max_cpu_percent: 70.0,
                max_memory_mb: 4096,
                priority: 7,
            },
        );
        resource_manager.set_agent_limits(
            "qwen-code",
            AgentLimits {
                max_concurrent_tasks: 2,
                max_cpu_percent: 60.0,
                max_memory_mb: 3072,
                priority: 9,
            },
        );

        Self {
            agents: Arc::new(Mutex::new(HashMap::new())),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            messages: Arc::new(Mutex::new(Vec::new())),
            skills_index: Arc::new(Mutex::new(HashMap::new())),
            resource_manager,
            legacy_files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn new_with_persistence(data_dir: &Path) -> Self {
        let orch = Self::new();
        orch.load(data_dir);
        let limits_path = data_dir.join("resource_limits.json");
        if limits_path.exists() {
            let _ = orch.resource_manager.load_limits(&limits_path);
        }
        orch
    }

    pub fn save(&self, data_dir: &Path) {
        let agents = self.agents.lock_safe();
        let tasks = self.tasks.lock_safe();
        if let Ok(data) = serde_json::to_string_pretty(&*agents) {
            let _ = std::fs::write(data_dir.join("orch_agents.json"), data);
        }
        if let Ok(data) = serde_json::to_string_pretty(&*tasks) {
            let _ = std::fs::write(data_dir.join("orch_tasks.json"), data);
        }
    }

    fn load(&self, data_dir: &Path) {
        if let Ok(data) = std::fs::read_to_string(data_dir.join("orch_agents.json")) {
            if let Ok(agents) = serde_json::from_str::<HashMap<String, Agent>>(&data) {
                let mut a = self.agents.lock_safe();
                let mut index = self.skills_index.lock_safe();
                for (id, agent) in agents.iter() {
                    for skill in &agent.skills {
                        index.entry(skill.clone()).or_default().push(id.clone());
                    }
                }
                *a = agents;
            }
        }
        if let Ok(data) = std::fs::read_to_string(data_dir.join("orch_tasks.json")) {
            if let Ok(tasks) = serde_json::from_str::<HashMap<String, Task>>(&data) {
                *self.tasks.lock_safe() = tasks;
            }
        }
    }

    pub fn get_system_status(&self) -> serde_json::Value {
        let agents = self.agents.lock_safe();
        let tasks = self.tasks.lock_safe();
        let files = self.legacy_files.lock_safe();
        let sub_orchs: Vec<&Agent> = agents.values().filter(|a| a.is_sub_orchestrator).collect();
        serde_json::json!({
            "agents": {
                "total": agents.len(),
                "idle": agents.values().filter(|a| a.status == AgentStatus::Idle).count(),
                "busy": agents.values().filter(|a| a.status == AgentStatus::Busy).count(),
                "sub_orchestrators": sub_orchs.len(),
                "list": agents.values().map(|a| serde_json::json!({
                    "id": a.id, "type": a.agent_type, "status": format!("{:?}", a.status),
                    "skills": a.skills, "workload": a.workload,
                    "is_sub_orch": a.is_sub_orchestrator,
                    "parent": a.parent_agent,
                    "sub_agents": a.sub_agents,
                })).collect::<Vec<_>>()
            },
            "tasks": {
                "pending": tasks.values().filter(|t| t.status == TaskStatus::Pending).count(),
                "in_progress": tasks.values().filter(|t| t.status == TaskStatus::InProgress).count(),
                "completed": tasks.values().filter(|t| t.status == TaskStatus::Completed).count(),
                "awaiting_review": tasks.values().filter(|t| t.status == TaskStatus::AwaitingReview).count(),
                "coordinated": tasks.values().filter(|t| t.coordinated).count(),
            },
            "legacy_files": files.len(),
            "timestamp": timestamp_now()
        })
    }

    pub fn cleanup_stale_agents(&self, timeout_secs: u64) {
        let now = timestamp_now();
        let timeout = timeout_secs as i64;
        let stale: Vec<String> = self
            .agents
            .lock_safe()
            .iter()
            .filter(|(_, a)| now - a.last_heartbeat > timeout)
            .map(|(id, _)| id.clone())
            .collect();
        for id in stale {
            self.unregister_agent(&id);
        }
    }
}

pub(crate) fn timestamp_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
