//! Synapsis Orchestrator - Multi-Agent Coordination Engine
//!
//! Central orchestration layer for all agents. Handles:
//! - Agent registration and lifecycle
//! - Task distribution and load balancing
//! - Skill-based routing
//! - Communication routing
//! - Proactive task assignment (agents get work automatically)
//!
//! PROACTIVE MODE: When an agent registers or heartbeats with Idle status,
//! the orchestrator automatically assigns pending tasks to them.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::core::uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Agent {
    pub id: String,
    pub agent_type: String,
    pub name: String,
    pub skills: Vec<String>,
    pub status: AgentStatus,
    pub current_task: Option<String>,
    pub workload: u32,
    pub created_at: i64,
    pub last_heartbeat: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentStatus {
    Idle,
    Busy,
    Thinking,
    Waiting,
    Disconnected,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub required_skills: Vec<String>,
    pub priority: u8,
    pub assigned_to: Option<String>,
    pub status: TaskStatus,
    pub created_at: i64,
    pub parent_task: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaskStatus {
    Pending,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Delegated,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrchestratorMessage {
    pub id: String,
    pub from: String,
    pub to: Option<String>,
    pub message_type: MessageType,
    pub payload: serde_json::Value,
    pub timestamp: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MessageType {
    TaskRequest,
    TaskResponse,
    Delegation,
    SkillOffer,
    SkillRequest,
    Heartbeat,
    StatusUpdate,
    Coordination,
}

pub struct Orchestrator {
    agents: Arc<Mutex<HashMap<String, Agent>>>,
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    messages: Arc<Mutex<Vec<OrchestratorMessage>>>,
    skills_index: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(Mutex::new(HashMap::new())),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            messages: Arc::new(Mutex::new(Vec::new())),
            skills_index: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn new_with_persistence(data_dir: &Path) -> Self {
        let orch = Self::new();
        orch.load(data_dir);
        orch
    }

    pub fn save(&self, data_dir: &Path) {
        let agents = self.agents.lock().unwrap();
        let tasks = self.tasks.lock().unwrap();

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
                let mut a = self.agents.lock().unwrap();
                let mut index = self.skills_index.lock().unwrap();
                for (id, agent) in agents.iter() {
                    for skill in &agent.skills {
                        index
                            .entry(skill.clone())
                            .or_insert_with(Vec::new)
                            .push(id.clone());
                    }
                }
                *a = agents;
            }
        }
        if let Ok(data) = std::fs::read_to_string(data_dir.join("orch_tasks.json")) {
            if let Ok(tasks) = serde_json::from_str::<HashMap<String, Task>>(&data) {
                let mut t = self.tasks.lock().unwrap();
                *t = tasks;
            }
        }
    }

    pub fn register_agent(&self, agent_type: &str, skills: Vec<String>) -> String {
        let id = format!("{}-{}", agent_type, Uuid::new_v4().to_hex_string());
        let now = timestamp_now();

        let agent = Agent {
            id: id.clone(),
            agent_type: agent_type.to_string(),
            name: format!("{}_{}", agent_type, &id[..8]),
            skills: skills.clone(),
            status: AgentStatus::Idle,
            current_task: None,
            workload: 0,
            created_at: now,
            last_heartbeat: now,
        };

        self.agents.lock().unwrap().insert(id.clone(), agent);

        for skill in &skills {
            let mut index = self.skills_index.lock().unwrap();
            index
                .entry(skill.clone())
                .or_insert_with(Vec::new)
                .push(id.clone());
        }

        self.log_message(
            &id,
            None,
            MessageType::Coordination,
            serde_json::json!({"action": "registered", "skills": skills}),
        );

        id
    }

    pub fn register_agent_with_id(&self, agent_id: &str, agent_type: &str, skills: Vec<String>) {
        let now = timestamp_now();

        let agent = Agent {
            id: agent_id.to_string(),
            agent_type: agent_type.to_string(),
            name: format!("{}_{}", agent_type, &agent_id[..8]),
            skills: skills.clone(),
            status: AgentStatus::Idle,
            current_task: None,
            workload: 0,
            created_at: now,
            last_heartbeat: now,
        };

        self.agents
            .lock()
            .unwrap()
            .insert(agent_id.to_string(), agent);

        for skill in &skills {
            let mut index = self.skills_index.lock().unwrap();
            index
                .entry(skill.clone())
                .or_insert_with(Vec::new)
                .push(agent_id.to_string());
        }

        self.log_message(
            agent_id,
            None,
            MessageType::Coordination,
            serde_json::json!({"action": "registered", "skills": skills}),
        );
    }

    pub fn unregister_agent(&self, agent_id: &str) {
        let mut agents = self.agents.lock().unwrap();
        if let Some(agent) = agents.remove(agent_id) {
            let mut index = self.skills_index.lock().unwrap();
            for skill in &agent.skills {
                if let Some(agent_list) = index.get_mut(skill) {
                    agent_list.retain(|a| a != agent_id);
                }
            }
        }
    }

    pub fn heartbeat(&self, agent_id: &str, status: Option<AgentStatus>, task: Option<&str>) {
        let was_idle = {
            let agents = self.agents.lock().unwrap();
            agents
                .get(agent_id)
                .map(|a| a.status == AgentStatus::Idle)
                .unwrap_or(false)
        };

        let mut agents = self.agents.lock().unwrap();
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.last_heartbeat = timestamp_now();
            if let Some(s) = status {
                agent.status = s;
            }
            if let Some(t) = task {
                agent.current_task = Some(t.to_string());
            }
        }
        drop(agents);

        if was_idle || status == Some(AgentStatus::Idle) {
            self.proactive_assign_to(agent_id);
        }
    }

    pub fn proactive_assign_to(&self, agent_id: &str) -> Option<Task> {
        let (agent_status, agent_skills) = {
            let agents = self.agents.lock().unwrap();
            match agents.get(agent_id) {
                Some(a) => (a.status.clone(), a.skills.clone()),
                None => return None,
            }
        };

        if agent_status != AgentStatus::Idle {
            return None;
        }

        let pending = self.get_pending_tasks();
        for task in pending {
            if task
                .required_skills
                .iter()
                .any(|s| agent_skills.contains(s))
            {
                if self.assign_task(&task.id, agent_id) {
                    return Some(task);
                }
            }
        }
        None
    }

    pub fn proactive_assign_all(&self) -> Vec<(String, Task)> {
        let mut assigned = Vec::new();
        let idle_agents = self.get_idle_agents();

        for agent in idle_agents {
            if let Some(task) = self.proactive_assign_to(&agent.id) {
                assigned.push((agent.id.clone(), task));
            }
        }

        assigned
    }

    pub fn get_agent_task_notification(&self, agent_id: &str) -> Option<serde_json::Value> {
        let messages = self.get_agent_messages(agent_id, 0);
        messages
            .into_iter()
            .find(|m| matches!(m.message_type, MessageType::TaskResponse))
            .map(|m| m.payload)
    }

    pub fn create_task(
        &self,
        description: &str,
        required_skills: Vec<String>,
        priority: u8,
        parent: Option<&str>,
    ) -> String {
        let id = format!("task-{}", Uuid::new_v4().to_hex_string());
        let now = timestamp_now();

        let task = Task {
            id: id.clone(),
            description: description.to_string(),
            required_skills,
            priority,
            assigned_to: None,
            status: TaskStatus::Pending,
            created_at: now,
            parent_task: parent.map(String::from),
        };

        self.tasks.lock().unwrap().insert(id.clone(), task);

        id
    }

    pub fn find_best_agent(&self, skills_needed: &[String]) -> Option<String> {
        let agents = self.agents.lock().unwrap();

        let mut candidates: Vec<&Agent> = agents
            .values()
            .filter(|a| a.status == AgentStatus::Idle || a.status == AgentStatus::Thinking)
            .filter(|a| skills_needed.iter().any(|s| a.skills.contains(s)))
            .collect();

        candidates.sort_by_key(|a| a.workload);

        candidates.first().map(|a| a.id.clone())
    }

    pub fn assign_task(&self, task_id: &str, agent_id: &str) -> bool {
        let task_desc = {
            let mut tasks = self.tasks.lock().unwrap();
            if let Some(task) = tasks.get_mut(task_id) {
                task.status = TaskStatus::Assigned;
                task.assigned_to = Some(agent_id.to_string());
                Some(task.description.clone())
            } else {
                None
            }
        };

        if let Some(desc) = task_desc {
            let mut agents = self.agents.lock().unwrap();
            if let Some(agent) = agents.get_mut(agent_id) {
                agent.status = AgentStatus::Busy;
                agent.current_task = Some(task_id.to_string());
                agent.workload += 1;
            }
            drop(agents);

            self.log_message(
                "orchestrator",
                Some(agent_id),
                MessageType::TaskResponse,
                serde_json::json!({
                    "action": "task_assigned",
                    "task_id": task_id,
                    "description": desc,
                    "priority": self.tasks.lock().unwrap().get(task_id).map(|t| t.priority).unwrap_or(0)
                }),
            );
            true
        } else {
            false
        }
    }

    pub fn complete_task(&self, task_id: &str, success: bool) {
        let agent_id = {
            let mut tasks = self.tasks.lock().unwrap();
            if let Some(task) = tasks.get_mut(task_id) {
                task.status = if success {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Failed
                };
                task.assigned_to.clone()
            } else {
                None
            }
        };

        if let Some(aid) = agent_id {
            let mut agents = self.agents.lock().unwrap();
            if let Some(agent) = agents.get_mut(&aid) {
                agent.status = AgentStatus::Idle;
                agent.current_task = None;
                agent.workload = agent.workload.saturating_sub(1);
            }
        }
    }

    pub fn delegate_task(&self, task_id: &str, from_agent: &str) -> Option<String> {
        let task = {
            let tasks = self.tasks.lock().unwrap();
            tasks.get(task_id).cloned()
        }?;

        if let Some(best_agent) = self.find_best_agent(&task.required_skills) {
            if best_agent != from_agent {
                self.assign_task(task_id, &best_agent);

                self.log_message(
                    from_agent,
                    Some(&best_agent),
                    MessageType::Delegation,
                    serde_json::json!({"task_id": task_id, "description": task.description}),
                );

                return Some(best_agent);
            }
        }

        None
    }

    pub fn send_message(
        &self,
        from: &str,
        to: Option<&str>,
        msg_type: MessageType,
        payload: serde_json::Value,
    ) -> String {
        let id = format!("msg-{}", Uuid::new_v4().to_hex_string());

        let msg = OrchestratorMessage {
            id: id.clone(),
            from: from.to_string(),
            to: to.map(String::from),
            message_type: msg_type,
            payload,
            timestamp: timestamp_now(),
        };

        self.messages.lock().unwrap().push(msg);

        id
    }

    pub fn get_agent_messages(&self, agent_id: &str, since: i64) -> Vec<OrchestratorMessage> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter(|m| {
                m.timestamp > since && (m.to.as_deref() == Some(agent_id) || m.to.is_none())
            })
            .cloned()
            .collect()
    }

    pub fn get_idle_agents(&self) -> Vec<Agent> {
        self.agents
            .lock()
            .unwrap()
            .values()
            .filter(|a| a.status == AgentStatus::Idle)
            .cloned()
            .collect()
    }

    pub fn get_pending_tasks(&self) -> Vec<Task> {
        self.tasks
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.status == TaskStatus::Pending)
            .cloned()
            .collect()
    }

    pub fn get_system_status(&self) -> serde_json::Value {
        let agents = self.agents.lock().unwrap();
        let tasks = self.tasks.lock().unwrap();

        serde_json::json!({
            "agents": {
                "total": agents.len(),
                "idle": agents.values().filter(|a| a.status == AgentStatus::Idle).count(),
                "busy": agents.values().filter(|a| a.status == AgentStatus::Busy).count(),
                "list": agents.values().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "type": a.agent_type,
                        "status": format!("{:?}", a.status),
                        "skills": a.skills,
                        "workload": a.workload
                    })
                }).collect::<Vec<_>>()
            },
            "tasks": {
                "pending": tasks.values().filter(|t| t.status == TaskStatus::Pending).count(),
                "in_progress": tasks.values().filter(|t| t.status == TaskStatus::InProgress).count(),
                "completed": tasks.values().filter(|t| t.status == TaskStatus::Completed).count(),
            },
            "timestamp": timestamp_now()
        })
    }

    pub fn log_message(
        &self,
        from: &str,
        to: Option<&str>,
        msg_type: MessageType,
        payload: serde_json::Value,
    ) {
        self.send_message(from, to, msg_type, payload);
    }

    pub fn cleanup_stale_agents(&self, timeout_secs: u64) {
        let now = timestamp_now();
        let timeout = timeout_secs as i64;

        let stale: Vec<String> = {
            let agents = self.agents.lock().unwrap();
            agents
                .iter()
                .filter(|(_, a)| now - a.last_heartbeat > timeout)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in stale {
            self.unregister_agent(&id);
        }
    }
}

fn timestamp_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
