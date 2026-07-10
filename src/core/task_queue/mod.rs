mod types;

use crate::core::lock_utils::*;
use std::collections::{BinaryHeap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tokio::time::{Duration, interval};

use types::PriorityTask;
pub use types::{AgentInfo, Priority, Task, TaskQueueEvent, TaskQueueEventType, TaskStatus};

pub struct TaskQueue {
    pending_queue: Arc<RwLock<BinaryHeap<PriorityTask>>>,
    assigned_tasks: Arc<RwLock<HashMap<String, Task>>>,
    completed_tasks: Arc<RwLock<HashMap<String, Task>>>,
    agents: Arc<RwLock<HashMap<String, AgentInfo>>>,
    task_order: AtomicU64,
    event_sender: broadcast::Sender<TaskQueueEvent>,
    running: AtomicBool,
    heartbeat_timeout_secs: u64,
    data_dir: PathBuf,
}

impl TaskQueue {
    pub fn new(data_dir: Option<PathBuf>) -> Self {
        let (tx, _) = broadcast::channel(1000);
        let data_dir = data_dir.unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("synapsis")
                .join("task_queue")
        });

        let queue = Self {
            pending_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            assigned_tasks: Arc::new(RwLock::new(HashMap::new())),
            completed_tasks: Arc::new(RwLock::new(HashMap::new())),
            agents: Arc::new(RwLock::new(HashMap::new())),
            task_order: AtomicU64::new(0),
            event_sender: tx,
            running: AtomicBool::new(true),
            heartbeat_timeout_secs: 30,
            data_dir,
        };

        std::fs::create_dir_all(&queue.data_dir).ok();
        queue
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TaskQueueEvent> {
        self.event_sender.subscribe()
    }

    fn emit(&self, event: TaskQueueEvent) {
        let _ = self.event_sender.send(event);
    }

    pub fn set_heartbeat_timeout(&mut self, secs: u64) {
        self.heartbeat_timeout_secs = secs;
    }

    pub fn push_task(&self, mut task: Task) -> String {
        let order = self.task_order.fetch_add(1, AtomicOrdering::Relaxed);
        task.status = TaskStatus::Pending;
        let task_id = task.id.clone();

        {
            let mut queue = self.pending_queue.write_safe();
            queue.push(PriorityTask::new(task, order));
        }

        self.emit(TaskQueueEvent {
            event_type: TaskQueueEventType::TaskCreated,
            task_id: task_id.clone(),
            agent_id: None,
            timestamp: crate::domain::types::Timestamp::now(),
            payload: None,
        });

        self.try_auto_assign();
        let _ = self.save();

        task_id
    }

    pub fn create_task(
        &self,
        description: String,
        required_skills: Vec<String>,
        priority: Priority,
    ) -> String {
        let task = Task::new(description, required_skills, priority);
        self.push_task(task)
    }

    pub fn register_agent(
        &self,
        agent_id: String,
        agent_type: String,
        skills: Vec<String>,
        capacity: u32,
    ) {
        let agent = AgentInfo::new(agent_id.clone(), agent_type, skills, capacity);
        {
            let mut agents = self.agents.write_safe();
            agents.insert(agent_id.clone(), agent);
        }

        self.emit(TaskQueueEvent {
            event_type: TaskQueueEventType::AgentRegistered,
            task_id: String::new(),
            agent_id: Some(agent_id.clone()),
            timestamp: crate::domain::types::Timestamp::now(),
            payload: None,
        });

        self.try_auto_assign();
    }

    pub fn heartbeat(&self, agent_id: &str) {
        let was_available = {
            let mut agents = self.agents.write_safe();
            if let Some(agent) = agents.get_mut(agent_id) {
                agent.last_heartbeat = crate::domain::types::Timestamp::now();
                agent.is_available = true;
                agent.is_available
            } else {
                false
            }
        };

        if was_available {
            self.emit(TaskQueueEvent {
                event_type: TaskQueueEventType::AgentHeartbeat,
                task_id: String::new(),
                agent_id: Some(agent_id.to_string()),
                timestamp: crate::domain::types::Timestamp::now(),
                payload: None,
            });
        }
    }

    pub fn unregister_agent(&self, agent_id: &str) {
        let tasks_to_requeue: Vec<Task> = {
            let mut agents = self.agents.write_safe();
            agents.remove(agent_id);
            Vec::new()
        };

        for mut task in tasks_to_requeue {
            task.assigned_to = None;
            task.status = TaskStatus::Pending;
            task.retry_count += 1;
            self.push_task(task);
        }
    }

    pub fn start_task(&self, task_id: &str, agent_id: &str) -> bool {
        let (should_emit, _task) = {
            let mut assigned = self.assigned_tasks.write_safe();
            if let Some(task) = assigned.get_mut(task_id) {
                if task.assigned_to.as_deref() == Some(agent_id) {
                    task.status = TaskStatus::InProgress;
                    self.emit(TaskQueueEvent {
                        event_type: TaskQueueEventType::TaskStarted,
                        task_id: task_id.to_string(),
                        agent_id: Some(agent_id.to_string()),
                        timestamp: crate::domain::types::Timestamp::now(),
                        payload: None,
                    });
                    (true, task.clone())
                } else {
                    return false;
                }
            } else {
                return false;
            }
        };

        if should_emit {
            let _ = self.save();
        }
        true
    }

    pub fn complete_task(&self, task_id: &str, result: Option<String>, success: bool) -> bool {
        let _aid = {
            let mut assigned = self.assigned_tasks.write_safe();
            if let Some(mut task) = assigned.remove(task_id) {
                task.result = result;
                task.status = if success {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Failed
                };

                let prev_aid = task.assigned_to.clone();
                drop(assigned);

                if let Some(a) = &prev_aid {
                    let mut agents = self.agents.write_safe();
                    if let Some(agent) = agents.get_mut(a) {
                        agent.current_load = agent.current_load.saturating_sub(1);
                    }
                }

                let _t = task.clone();
                {
                    let mut completed = self.completed_tasks.write_safe();
                    completed.insert(task_id.to_string(), task);
                }

                self.emit(TaskQueueEvent {
                    event_type: if success {
                        TaskQueueEventType::TaskCompleted
                    } else {
                        TaskQueueEventType::TaskFailed
                    },
                    task_id: task_id.to_string(),
                    agent_id: prev_aid.clone(),
                    timestamp: crate::domain::types::Timestamp::now(),
                    payload: None,
                });

                prev_aid
            } else {
                return false;
            }
        };

        let _ = self.save();
        self.try_auto_assign();
        true
    }

    pub fn get_pending_tasks(&self) -> Vec<Task> {
        let queue = self.pending_queue.read_safe();
        queue.iter().map(|pt| pt.task.clone()).collect()
    }

    pub fn get_assigned_tasks(&self) -> Vec<Task> {
        let assigned = self.assigned_tasks.read_safe();
        assigned.values().cloned().collect()
    }

    pub fn get_task(&self, task_id: &str) -> Option<Task> {
        {
            let queue = self.pending_queue.read_safe();
            if let Some(pt) = queue.iter().find(|pt| pt.task.id == task_id) {
                return Some(pt.task.clone());
            }
        }
        {
            let assigned = self.assigned_tasks.read_safe();
            if let Some(t) = assigned.get(task_id) {
                return Some(t.clone());
            }
        }
        {
            let completed = self.completed_tasks.read_safe();
            if let Some(t) = completed.get(task_id) {
                return Some(t.clone());
            }
        }
        None
    }

    pub fn get_agent(&self, agent_id: &str) -> Option<AgentInfo> {
        let agents = self.agents.read_safe();
        agents.get(agent_id).cloned()
    }

    pub fn get_idle_agents(&self) -> Vec<AgentInfo> {
        let agents = self.agents.read_safe();
        agents
            .values()
            .filter(|a| a.has_capacity())
            .cloned()
            .collect()
    }

    pub fn get_queue_stats(&self) -> serde_json::Value {
        let pending = {
            let q = self.pending_queue.read_safe();
            q.len()
        };
        let assigned = {
            let a = self.assigned_tasks.read_safe();
            a.len()
        };
        let completed = {
            let c = self.completed_tasks.read_safe();
            c.len()
        };
        let agents = {
            let a = self.agents.read_safe();
            a.len()
        };
        let idle_agents = {
            let a = self.agents.read_safe();
            a.values().filter(|ag| ag.has_capacity()).count()
        };

        serde_json::json!({
            "pending": pending,
            "assigned": assigned,
            "completed": completed,
            "total_agents": agents,
            "idle_agents": idle_agents,
        })
    }

    fn try_auto_assign(&self) {
        let available_agents = self.get_idle_agents();
        if available_agents.is_empty() {
            return;
        }

        let tasks_to_assign: Vec<(Task, String)> = {
            let mut queue = self.pending_queue.write_safe();
            let mut tasks_to_assign = Vec::new();

            while let Some(pt) = queue.pop() {
                if pt.task.is_expired() && !pt.task.can_retry() {
                    let mut failed = self.assigned_tasks.write_safe();
                    let mut task = pt.task;
                    task.status = TaskStatus::Failed;
                    failed.insert(task.id.clone(), task);
                    continue;
                }

                let best_agent = self.find_best_agent(&pt.task, &available_agents);
                if let Some(agent_id) = best_agent {
                    let mut task = pt.task;
                    task.status = TaskStatus::Assigned;
                    task.assigned_to = Some(agent_id.clone());
                    tasks_to_assign.push((task, agent_id));
                } else {
                    queue.push(pt);
                    break;
                }
            }
            tasks_to_assign
        };

        for (task, agent_id) in tasks_to_assign {
            {
                let mut assigned = self.assigned_tasks.write_safe();
                assigned.insert(task.id.clone(), task.clone());
            }
            {
                let mut agents = self.agents.write_safe();
                if let Some(agent) = agents.get_mut(&agent_id) {
                    agent.current_load += 1;
                }
            }

            self.emit(TaskQueueEvent {
                event_type: TaskQueueEventType::TaskAssigned,
                task_id: task.id,
                agent_id: Some(agent_id),
                timestamp: crate::domain::types::Timestamp::now(),
                payload: None,
            });
        }
    }

    fn find_best_agent(&self, task: &Task, agents: &[AgentInfo]) -> Option<String> {
        let mut candidates: Vec<(String, i64)> = agents
            .iter()
            .filter(|a| a.has_capacity() && a.matches_skills(&task.required_skills))
            .map(|a| {
                let skill_score = a.skill_score(&task.required_skills) as i64;
                let load_penalty = a.current_load as i64;
                let priority = -(skill_score * 100 - load_penalty * 10);
                (a.id.clone(), priority)
            })
            .collect();

        candidates.sort_by_key(|a| a.1);

        candidates.into_iter().next().map(|(id, _)| id)
    }

    pub fn check_agent_timeouts(&self) -> Vec<String> {
        let now = crate::domain::types::Timestamp::now();
        let timeout_threshold = self.heartbeat_timeout_secs as i64;
        let mut timed_out_agents = Vec::new();

        let tasks_to_requeue: Vec<Task> = {
            let mut agents = self.agents.write_safe();
            let mut to_requeue = Vec::new();

            for (agent_id, agent) in agents.iter_mut() {
                let elapsed = now.0 - agent.last_heartbeat.0;
                if elapsed > timeout_threshold && agent.is_available {
                    agent.is_available = false;
                    timed_out_agents.push(agent_id.clone());

                    self.emit(TaskQueueEvent {
                        event_type: TaskQueueEventType::AgentTimeout,
                        task_id: String::new(),
                        agent_id: Some(agent_id.clone()),
                        timestamp: crate::domain::types::Timestamp::now(),
                        payload: Some(serde_json::json!({
                            "last_heartbeat": agent.last_heartbeat.0,
                            "elapsed_secs": elapsed
                        })),
                    });
                }
            }

            drop(agents);

            if !timed_out_agents.is_empty() {
                let mut assigned = self.assigned_tasks.write_safe();
                let mut requeued = Vec::new();

                for agent_id in &timed_out_agents {
                    let tasks: Vec<Task> = assigned
                        .values()
                        .filter(|t| t.assigned_to.as_deref() == Some(agent_id))
                        .cloned()
                        .collect();

                    for task in tasks {
                        let task_id = task.id.clone();
                        let retry_count = task.retry_count + 1;
                        assigned.remove(&task_id);
                        let mut t = task;
                        t.assigned_to = None;
                        t.status = TaskStatus::Pending;
                        t.retry_count = retry_count;

                        if t.can_retry() {
                            requeued.push(t);
                            self.emit(TaskQueueEvent {
                                event_type: TaskQueueEventType::TaskRequeued,
                                task_id: task_id.clone(),
                                agent_id: Some(agent_id.clone()),
                                timestamp: crate::domain::types::Timestamp::now(),
                                payload: Some(serde_json::json!({
                                    "retry_count": retry_count
                                })),
                            });
                        } else {
                            t.status = TaskStatus::Failed;
                            let mut completed = self.completed_tasks.write_safe();
                            completed.insert(task_id, t);
                        }
                    }
                }
                to_requeue = requeued;
            }
            to_requeue
        };

        for task in tasks_to_requeue {
            self.push_task(task);
        }

        timed_out_agents
    }

    pub fn cancel_task(&self, task_id: &str) -> bool {
        {
            let mut queue = self.pending_queue.write_safe();
            let old: Vec<PriorityTask> = queue.drain().collect();
            for pt in old {
                if pt.task.id != task_id {
                    queue.push(pt);
                }
            }
        }

        let (agent_id, was_assigned) = {
            let mut assigned = self.assigned_tasks.write_safe();
            if let Some(task) = assigned.remove(task_id) {
                (task.assigned_to.clone(), true)
            } else {
                (None, false)
            }
        };

        if let Some(aid) = agent_id {
            let mut agents = self.agents.write_safe();
            if let Some(agent) = agents.get_mut(&aid) {
                agent.current_load = agent.current_load.saturating_sub(1);
            }
        }

        let mut completed = self.completed_tasks.write_safe();
        completed.insert(
            task_id.to_string(),
            Task {
                id: task_id.to_string(),
                description: String::new(),
                required_skills: Vec::new(),
                priority: Priority::Normal,
                status: TaskStatus::Cancelled,
                assigned_to: None,
                created_at: crate::domain::types::Timestamp::now(),
                deadline: None,
                timeout_secs: None,
                result: None,
                retry_count: 0,
                max_retries: 0,
            },
        );

        let _ = self.save();
        was_assigned
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;

        let pending: Vec<Task> = {
            let q = self.pending_queue.read_safe();
            q.iter().map(|pt| pt.task.clone()).collect()
        };

        let assigned: Vec<Task> = {
            let a = self.assigned_tasks.read_safe();
            a.values().cloned().collect()
        };

        let completed: Vec<Task> = {
            let c = self.completed_tasks.read_safe();
            c.values().cloned().collect()
        };

        let agents: Vec<AgentInfo> = {
            let a = self.agents.read_safe();
            a.values().cloned().collect()
        };

        serde_json::to_writer(
            std::fs::File::create(self.data_dir.join("pending.json"))?,
            &pending,
        )?;
        serde_json::to_writer(
            std::fs::File::create(self.data_dir.join("assigned.json"))?,
            &assigned,
        )?;
        serde_json::to_writer(
            std::fs::File::create(self.data_dir.join("completed.json"))?,
            &completed,
        )?;
        serde_json::to_writer(
            std::fs::File::create(self.data_dir.join("agents.json"))?,
            &agents,
        )?;

        Ok(())
    }

    pub fn load(&self) -> std::io::Result<()> {
        if let Ok(file) = std::fs::File::open(self.data_dir.join("pending.json")) {
            if let Ok(pending) = serde_json::from_reader::<_, Vec<Task>>(file) {
                let mut queue = self.pending_queue.write_safe();
                let mut order = 0u64;
                for task in pending {
                    queue.push(PriorityTask::new(task, order));
                    order += 1;
                }
                self.task_order.store(order, AtomicOrdering::Relaxed);
            }
        }

        if let Ok(file) = std::fs::File::open(self.data_dir.join("assigned.json")) {
            if let Ok(assigned) = serde_json::from_reader::<_, Vec<Task>>(file) {
                let mut a = self.assigned_tasks.write_safe();
                for task in assigned {
                    a.insert(task.id.clone(), task);
                }
            }
        }

        if let Ok(file) = std::fs::File::open(self.data_dir.join("completed.json")) {
            if let Ok(completed) = serde_json::from_reader::<_, Vec<Task>>(file) {
                let mut c = self.completed_tasks.write_safe();
                for task in completed {
                    c.insert(task.id.clone(), task);
                }
            }
        }

        if let Ok(file) = std::fs::File::open(self.data_dir.join("agents.json")) {
            if let Ok(agents) = serde_json::from_reader::<_, Vec<AgentInfo>>(file) {
                let mut a = self.agents.write_safe();
                for agent in agents {
                    a.insert(agent.id.clone(), agent);
                }
            }
        }

        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, AtomicOrdering::SeqCst);
        let _ = self.save();
    }
}

impl Clone for TaskQueue {
    fn clone(&self) -> Self {
        Self {
            pending_queue: self.pending_queue.clone(),
            assigned_tasks: self.assigned_tasks.clone(),
            completed_tasks: self.completed_tasks.clone(),
            agents: self.agents.clone(),
            task_order: AtomicU64::new(self.task_order.load(AtomicOrdering::Relaxed)),
            event_sender: self.event_sender.clone(),
            running: AtomicBool::new(self.running.load(AtomicOrdering::SeqCst)),
            heartbeat_timeout_secs: self.heartbeat_timeout_secs,
            data_dir: self.data_dir.clone(),
        }
    }
}

pub struct TaskQueueManager {
    queue: Arc<TaskQueue>,
    running: AtomicBool,
}

impl TaskQueueManager {
    pub fn new(data_dir: Option<PathBuf>) -> Self {
        Self {
            queue: Arc::new(TaskQueue::new(data_dir)),
            running: AtomicBool::new(false),
        }
    }

    pub fn queue(&self) -> Arc<TaskQueue> {
        self.queue.clone()
    }

    pub async fn start(self: Arc<Self>) {
        if self.running.swap(true, AtomicOrdering::SeqCst) {
            return;
        }

        let queue = self.queue.clone();
        let timeout_check_interval = Duration::from_secs(5);

        tokio::spawn(async move {
            let mut ticker = interval(timeout_check_interval);
            while queue.running.load(AtomicOrdering::SeqCst) {
                ticker.tick().await;
                queue.check_agent_timeouts();
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, AtomicOrdering::SeqCst);
        self.queue.stop();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let queue = TaskQueue::new(None);
        let task_id = queue.create_task(
            "Test task".to_string(),
            vec!["coding".to_string()],
            Priority::High,
        );
        assert!(!task_id.is_empty());
    }

    #[test]
    fn test_agent_registration_and_auto_assign() {
        let queue = TaskQueue::new(None);

        queue.register_agent(
            "agent-1".to_string(),
            "coder".to_string(),
            vec!["coding".to_string(), "debugging".to_string()],
            3,
        );

        let task_id = queue.create_task(
            "Write tests".to_string(),
            vec!["coding".to_string()],
            Priority::Normal,
        );

        let task = queue.get_task(&task_id);
        assert!(task.is_some());
        assert_eq!(task.unwrap().status, TaskStatus::Assigned);
    }

    #[test]
    fn test_heartbeat_system() {
        let mut queue = TaskQueue::new(None);
        queue.set_heartbeat_timeout(5);

        queue.register_agent(
            "agent-1".to_string(),
            "coder".to_string(),
            vec!["coding".to_string()],
            2,
        );

        queue.heartbeat("agent-1");
        assert!(queue.get_agent("agent-1").unwrap().is_available);

        queue.check_agent_timeouts();
        assert!(queue.get_agent("agent-1").unwrap().is_available);
    }

    #[test]
    fn test_task_requeue_on_timeout() {
        let mut queue = TaskQueue::new(None);
        queue.set_heartbeat_timeout(1);

        queue.register_agent(
            "agent-1".to_string(),
            "coder".to_string(),
            vec!["coding".to_string()],
            2,
        );

        let task_id = queue.create_task(
            "Test task".to_string(),
            vec!["coding".to_string()],
            Priority::Normal,
        );

        assert_eq!(
            queue.get_task(&task_id).unwrap().status,
            TaskStatus::Assigned
        );

        std::thread::sleep(std::time::Duration::from_secs(2));
        queue.check_agent_timeouts();

        let task = queue.get_task(&task_id);
        assert!(task.is_some());
    }
}
