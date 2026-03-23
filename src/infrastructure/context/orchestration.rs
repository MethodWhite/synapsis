//! # Orchestration System

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new(name: &str) -> Self {
        Self(format!(
            "{}_{:x}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new() -> Self {
        Self(format!(
            "task_{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    Orchestrator,
    Coder,
    Researcher,
    Reviewer,
    Tester,
    Architect,
    Security,
    Docs,
    DevOps,
}

impl AgentType {
    pub fn can_handle(&self, task: &TaskType) -> bool {
        match (self, task) {
            (AgentType::Orchestrator, _) => true,
            (AgentType::Coder, TaskType::Code) | (AgentType::Coder, TaskType::Refactor) => true,
            (AgentType::Researcher, TaskType::Research)
            | (AgentType::Researcher, TaskType::Analysis) => true,
            (AgentType::Reviewer, TaskType::Review) | (AgentType::Reviewer, TaskType::Code) => true,
            (AgentType::Tester, TaskType::Test) | (AgentType::Tester, TaskType::Benchmark) => true,
            (AgentType::Architect, TaskType::Design) | (AgentType::Architect, TaskType::Plan) => {
                true
            }
            (AgentType::Security, TaskType::SecurityAudit)
            | (AgentType::Security, TaskType::Scan) => true,
            (AgentType::Docs, TaskType::Documentation) => true,
            (AgentType::DevOps, TaskType::Deploy) | (AgentType::DevOps, TaskType::Configure) => {
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Code,
    Refactor,
    Review,
    Test,
    Benchmark,
    Research,
    Analysis,
    Design,
    Plan,
    SecurityAudit,
    Scan,
    Documentation,
    Deploy,
    Configure,
    Coordinate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Pending,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskPriority {
    Critical,
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub success: bool,
    pub summary: String,
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub task_type: TaskType,
    pub description: String,
    pub assigned_to: Option<AgentId>,
    pub state: TaskState,
    pub priority: TaskPriority,
    pub created_at: i64,
    pub result: Option<TaskResult>,
}

impl Task {
    pub fn new(task_type: TaskType, description: String) -> Self {
        Self {
            id: TaskId::new(),
            task_type,
            description,
            assigned_to: None,
            state: TaskState::Pending,
            priority: TaskPriority::Normal,
            created_at: now_ts(),
            result: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Working,
    Waiting,
    Available,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub agent_type: AgentType,
    pub state: AgentState,
    pub current_task: Option<TaskId>,
    pub completed_tasks: u64,
}

pub struct Orchestrator {
    agents: HashMap<AgentId, Agent>,
    active_tasks: HashMap<TaskId, Task>,
    metrics: OrchMetrics,
    decisions: Vec<Decision>,
}

#[derive(Default)]
struct OrchMetrics {
    total_tasks: u64,
    completed: u64,
    failed: u64,
    delegations: u64,
}


#[derive(Debug, Clone)]
struct Decision {
    timestamp: i64,
    reason: String,
    agent: Option<AgentId>,
    task: Option<TaskId>,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            active_tasks: HashMap::new(),
            metrics: OrchMetrics::default(),
            decisions: Vec::new(),
        }
    }

    pub fn register(&mut self, agent_type: AgentType, name: &str) -> AgentId {
        let id = AgentId::new(name);
        let agent = Agent {
            id: id.clone(),
            name: name.to_string(),
            agent_type,
            state: AgentState::Idle,
            current_task: None,
            completed_tasks: 0,
        };
        self.agents.insert(id.clone(), agent);
        self.log(
            &format!("Registered {:?} agent: {}", agent_type, name),
            Some(id.clone()),
            None,
        );
        id
    }

    pub fn plan_task(&mut self, task: Task) -> TaskId {
        let id = task.id.clone();
        self.active_tasks.insert(id.clone(), task);
        self.metrics.total_tasks += 1;
        self.log(&"Task planned".to_string(), None, Some(id.clone()));
        id
    }

    pub fn delegate(&mut self, task_id: &TaskId, agent_id: &AgentId) -> bool {
        let task = match self.active_tasks.get_mut(task_id) {
            Some(t) => t,
            None => return false,
        };
        let agent = match self.agents.get_mut(agent_id) {
            Some(a) => a,
            None => return false,
        };

        task.assigned_to = Some(agent_id.clone());
        task.state = TaskState::Assigned;
        agent.state = AgentState::Working;
        agent.current_task = Some(task_id.clone());
        self.metrics.delegations += 1;
        let agent_name = agent.name.clone();
        let _ = agent;
        self.log(
            &format!("Delegated to {}", agent_name),
            Some(agent_id.clone()),
            Some(task_id.clone()),
        );
        true
    }

    pub fn complete(&mut self, task_id: &TaskId, result: TaskResult) -> bool {
        let task = match self.active_tasks.get_mut(task_id) {
            Some(t) => t,
            None => return false,
        };
        let state = if result.success {
            TaskState::Completed
        } else {
            TaskState::Failed
        };
        let agent_id = task.assigned_to.clone();
        task.result = Some(result.clone());
        task.state = state;

        if let Some(aid) = &agent_id {
            if let Some(agent) = self.agents.get_mut(aid) {
                agent.state = AgentState::Idle;
                agent.current_task = None;
                agent.completed_tasks += 1;
            }
        }

        if result.success {
            self.metrics.completed += 1;
        } else {
            self.metrics.failed += 1;
        }
        self.log(
            &format!("Task {:?} {}", state, result.success),
            agent_id.clone(),
            Some(task_id.clone()),
        );
        true
    }

    pub fn recommend(&self, task: &Task) -> Vec<(AgentId, f64)> {
        let mut candidates: Vec<(AgentId, f64)> = self
            .agents
            .iter()
            .filter(|(_, a)| a.state == AgentState::Idle || a.state == AgentState::Available)
            .filter(|(_, a)| a.agent_type.can_handle(&task.task_type))
            .map(|(id, a)| {
                let mut score: f64 = 1.0;
                if a.state == AgentState::Working {
                    score *= 0.5;
                }
                score += (a.completed_tasks as f64 * 0.01).min(0.5);
                (id.clone(), score)
            })
            .collect();
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        candidates
    }

    pub fn status(&self) -> OrchStatus {
        OrchStatus {
            total_agents: self.agents.len(),
            active_agents: self
                .agents
                .values()
                .filter(|a| a.state == AgentState::Working)
                .count(),
            total_tasks: self.active_tasks.len(),
            pending_tasks: self
                .active_tasks
                .values()
                .filter(|t| t.state == TaskState::Pending)
                .count(),
            completed: self.metrics.completed,
            failed: self.metrics.failed,
        }
    }

    pub fn suggest(&self) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        let pending: usize = self
            .active_tasks
            .values()
            .filter(|t| t.state == TaskState::Pending)
            .count();
        let idle: usize = self
            .agents
            .values()
            .filter(|a| a.state == AgentState::Idle)
            .count();

        if pending > 0 {
            suggestions.push(Suggestion {
                action: format!("{} tasks need delegation", pending),
                priority: 1,
            });
        }
        if idle > 0 && pending > 0 {
            suggestions.push(Suggestion {
                action: format!("{} agents available", idle),
                priority: 2,
            });
        }

        suggestions
    }

    fn log(&mut self, reason: &str, agent: Option<AgentId>, task: Option<TaskId>) {
        self.decisions.push(Decision {
            timestamp: now_ts(),
            reason: reason.to_string(),
            agent,
            task,
        });
        if self.decisions.len() > 100 {
            self.decisions.remove(0);
        }
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct OrchStatus {
    pub total_agents: usize,
    pub active_agents: usize,
    pub total_tasks: usize,
    pub pending_tasks: usize,
    pub completed: u64,
    pub failed: u64,
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub action: String,
    pub priority: usize,
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
