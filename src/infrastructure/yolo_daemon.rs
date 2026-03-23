//! YOLO Daemon - Autonomous Agent Mode
//!
//! Runs an autonomous daemon that manages agents, tasks, and workers
//! with various levels of proactiveness based on the configured mode.

use crate::core::orchestrator::{Orchestrator, AgentStatus, Task as OrchTask, TaskStatus};
use crate::core::worker::{WorkerRegistry, WorkerAgent, Task, TaskResult, TaskError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::interval;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum YoloMode {
    Passive,
    Active,
    Aggressive,
    Stealth,
}

impl Default for YoloMode {
    fn default() -> Self {
        YoloMode::Passive
    }
}

impl std::fmt::Display for YoloMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            YoloMode::Passive => write!(f, "passive"),
            YoloMode::Active => write!(f, "active"),
            YoloMode::Aggressive => write!(f, "aggressive"),
            YoloMode::Stealth => write!(f, "stealth"),
        }
    }
}

impl std::str::FromStr for YoloMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "passive" => Ok(YoloMode::Passive),
            "active" => Ok(YoloMode::Active),
            "aggressive" => Ok(YoloMode::Aggressive),
            "stealth" => Ok(YoloMode::Stealth),
            _ => Err(format!("Unknown YOLO mode: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct YoloConfig {
    pub enabled: bool,
    pub mode: YoloMode,
    pub scan_interval_secs: u64,
    pub auto_connect: bool,
    pub max_workers: usize,
    pub auto_heal: bool,
}

impl Default for YoloConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: YoloMode::Passive,
            scan_interval_secs: 30,
            auto_connect: true,
            max_workers: 10,
            auto_heal: true,
        }
    }
}

pub struct YoloDaemon {
    config: YoloConfig,
    orchestrator: Arc<Orchestrator>,
    worker_registry: Arc<WorkerRegistry>,
    active_workers: Arc<Mutex<Vec<WorkerInfo>>>,
    pending_tasks: Arc<Mutex<Vec<PendingTask>>>,
    running: Arc<Mutex<bool>>,
    last_scan: Arc<Mutex<Instant>>,
}

#[derive(Debug, Clone)]
struct WorkerInfo {
    id: String,
    worker_type: String,
    status: WorkerDaemonStatus,
    last_heartbeat: Instant,
    restart_count: u32,
    current_task: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WorkerDaemonStatus {
    Running,
    Stopped,
    Failed,
    Restarting,
}

#[derive(Debug, Clone)]
struct PendingTask {
    id: String,
    description: String,
    skills_required: Vec<String>,
    priority: u8,
    retry_count: u32,
    created_at: Instant,
    assigned_to: Option<String>,
}

impl YoloDaemon {
    pub fn new(config: YoloConfig, orchestrator: Arc<Orchestrator>) -> Self {
        Self {
            config,
            orchestrator,
            worker_registry: Arc::new(WorkerRegistry::new()),
            active_workers: Arc::new(Mutex::new(Vec::new())),
            pending_tasks: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(Mutex::new(false)),
            last_scan: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn start(&self) {
        if self.config.enabled {
            *self.running.lock().unwrap() = true;
            let config = self.config.clone();
            let orchestrator = Arc::clone(&self.orchestrator);
            let worker_registry = Arc::clone(&self.worker_registry);
            let active_workers = Arc::clone(&self.active_workers);
            let pending_tasks = Arc::clone(&self.pending_tasks);
            let running = Arc::clone(&self.running);
            let last_scan = Arc::clone(&self.last_scan);

            tokio::spawn(async move {
                Self::run_daemon(
                    config,
                    orchestrator,
                    worker_registry,
                    active_workers,
                    pending_tasks,
                    running,
                    last_scan,
                ).await;
            });
        }
    }

    async fn run_daemon(
        config: YoloConfig,
        orchestrator: Arc<Orchestrator>,
        worker_registry: Arc<WorkerRegistry>,
        active_workers: Arc<Mutex<Vec<WorkerInfo>>>,
        pending_tasks: Arc<Mutex<Vec<PendingTask>>>,
        running: Arc<Mutex<bool>>,
        last_scan: Arc<Mutex<Instant>>,
    ) {
        let mut scan_interval = interval(Duration::from_secs(config.scan_interval_secs));

        loop {
            scan_interval.tick().await;

            if !*running.lock().unwrap() {
                break;
            }

            match config.mode {
                YoloMode::Passive => {
                    Self::execute_pending_tasks(&orchestrator, &pending_tasks, &active_workers);
                }
                YoloMode::Active => {
                    Self::scan_for_agents(&orchestrator, config.auto_connect);
                    Self::execute_pending_tasks(&orchestrator, &pending_tasks, &active_workers);
                    Self::monitor_workers(&worker_registry, &active_workers, config.auto_heal, config.max_workers);
                }
                YoloMode::Aggressive => {
                    Self::scan_for_agents(&orchestrator, config.auto_connect);
                    Self::auto_generate_tasks(&orchestrator);
                    Self::execute_pending_tasks(&orchestrator, &pending_tasks, &active_workers);
                    Self::auto_retry_failed(&pending_tasks, &orchestrator);
                    Self::monitor_workers(&worker_registry, &active_workers, config.auto_heal, config.max_workers);
                    Self::auto_scale_workers(&worker_registry, &active_workers, config.max_workers);
                }
                YoloMode::Stealth => {
                    Self::silent_monitor(&orchestrator, &pending_tasks, &active_workers);
                }
            }

            *last_scan.lock().unwrap() = Instant::now();
        }
    }

    fn scan_for_agents(orchestrator: &Arc<Orchestrator>, auto_connect: bool) {
        if auto_connect {
            let idle_agents = orchestrator.get_idle_agents();
            if !idle_agents.is_empty() {
                for agent in idle_agents {
                    let pending = orchestrator.get_pending_tasks();
                    for task in pending {
                        if task.required_skills.iter().any(|s| agent.skills.contains(s)) {
                            if orchestrator.assign_task(&task.id, &agent.id) {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    fn auto_generate_tasks(orchestrator: &Arc<Orchestrator>) {
        let pending = orchestrator.get_pending_tasks();
        if pending.len() < 3 {
            let task_id = orchestrator.create_task(
                "autonomous_check",
                vec!["monitor".to_string()],
                1,
                None,
            );
        }
    }

    fn execute_pending_tasks(
        orchestrator: &Arc<Orchestrator>,
        pending_tasks: &Arc<Mutex<Vec<PendingTask>>>,
        active_workers: &Arc<Mutex<Vec<WorkerInfo>>>,
    ) {
        let tasks = orchestrator.get_pending_tasks();
        let idle_agents = orchestrator.get_idle_agents();

        for task in tasks {
            for agent in &idle_agents {
                if task.required_skills.iter().any(|s| agent.skills.contains(s)) {
                    if orchestrator.assign_task(&task.id, &agent.id) {
                        let mut workers = active_workers.lock().unwrap();
                        if let Some(w) = workers.iter_mut().find(|w| w.id == agent.id) {
                            w.current_task = Some(task.id.clone());
                        }
                        break;
                    }
                }
            }
        }

        let mut pending = pending_tasks.lock().unwrap();
        pending.retain(|t| t.assigned_to.is_some() || t.retry_count < 3);
    }

    fn auto_retry_failed(
        pending_tasks: &Arc<Mutex<Vec<PendingTask>>>,
        orchestrator: &Arc<Orchestrator>,
    ) {
        let mut pending = pending_tasks.lock().unwrap();
        for task in pending.iter_mut() {
            if task.retry_count < 3 {
                task.retry_count += 1;
                let idle_agents = orchestrator.get_idle_agents();
                for agent in idle_agents {
                    if task.skills_required.iter().any(|s| agent.skills.contains(s)) {
                        if orchestrator.assign_task(&task.id, &agent.id) {
                            task.assigned_to = Some(agent.id.clone());
                            break;
                        }
                    }
                }
            }
        }
    }

    fn monitor_workers(
        worker_registry: &Arc<WorkerRegistry>,
        active_workers: &Arc<Mutex<Vec<WorkerInfo>>>,
        auto_heal: bool,
        _max_workers: usize,
    ) {
        let mut workers = active_workers.lock().unwrap();

        for worker in workers.iter_mut() {
            let is_alive = worker_registry.get(&worker.id)
                .map(|w| w.heartbeat())
                .unwrap_or(false);

            if !is_alive && auto_heal {
                if worker.restart_count < 5 {
                    worker.status = WorkerDaemonStatus::Restarting;
                    worker.restart_count += 1;
                    if let Some(w) = worker_registry.get(&worker.id) {
                        let _ = w.heartbeat();
                    }
                    worker.status = WorkerDaemonStatus::Running;
                } else {
                    worker.status = WorkerDaemonStatus::Failed;
                }
            }
        }

        workers.retain(|w| w.status != WorkerDaemonStatus::Failed);
    }

    fn auto_scale_workers(
        worker_registry: &Arc<WorkerRegistry>,
        active_workers: &Arc<Mutex<Vec<WorkerInfo>>>,
        max_workers: usize,
    ) {
        let pending = worker_registry.get(&"pending".to_string())
            .map(|_| true)
            .unwrap_or(false);

        let mut workers = active_workers.lock().unwrap();
        let current_count = workers.len();

        if pending && current_count < max_workers {
            workers.push(WorkerInfo {
                id: format!("scaled-{}", uuid::Uuid::new_v4().to_hex_string()),
                worker_type: "auto-scaled".to_string(),
                status: WorkerDaemonStatus::Running,
                last_heartbeat: Instant::now(),
                restart_count: 0,
                current_task: None,
            });
        }
    }

    fn silent_monitor(
        orchestrator: &Arc<Orchestrator>,
        pending_tasks: &Arc<Mutex<Vec<PendingTask>>>,
        active_workers: &Arc<Mutex<Vec<WorkerInfo>>>,
    ) {
        let _tasks = orchestrator.get_pending_tasks();
        let _workers = active_workers.lock().unwrap();
        let _pending = pending_tasks.lock().unwrap();
    }

    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    pub fn get_status(&self) -> YoloDaemonStatus {
        YoloDaemonStatus {
            running: self.is_running(),
            mode: self.config.mode,
            active_workers: self.active_workers.lock().unwrap().len(),
            pending_tasks: self.pending_tasks.lock().unwrap().len(),
            last_scan: *self.last_scan.lock().unwrap(),
        }
    }

    pub fn submit_task(&self, description: String, skills: Vec<String>, priority: u8) -> String {
        let task_id = format!("yolo-task-{}", uuid::Uuid::new_v4().to_hex_string());
        let pending = PendingTask {
            id: task_id.clone(),
            description,
            skills_required: skills,
            priority,
            retry_count: 0,
            created_at: Instant::now(),
            assigned_to: None,
        };

        self.pending_tasks.lock().unwrap().push(pending);

        if let Some(agent) = self.orchestrator.find_best_agent(&skills) {
            let _ = self.orchestrator.assign_task(&task_id, &agent);
        }

        task_id
    }

    pub fn queue_task(&self, task: PendingTask) {
        self.pending_tasks.lock().unwrap().push(task);
    }

    pub fn spawn_worker(&self, worker_type: String) -> String {
        let id = format!("{}-{}", worker_type, uuid::Uuid::new_v4().to_hex_string());

        let worker_info = WorkerInfo {
            id: id.clone(),
            worker_type: worker_type.clone(),
            status: WorkerDaemonStatus::Running,
            last_heartbeat: Instant::now(),
            restart_count: 0,
            current_task: None,
        };

        self.active_workers.lock().unwrap().push(worker_info);
        id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoloDaemonStatus {
    pub running: bool,
    pub mode: YoloMode,
    pub active_workers: usize,
    pub pending_tasks: usize,
    pub last_scan: Instant,
}

mod uuid {
    pub struct Uuid;

    impl Uuid {
        pub fn new_v4() -> Uuid {
            Uuid
        }

        pub fn to_hex_string(&self) -> String {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            (0..16).map(|_| format!("{:02x}", rng.gen::<u8>())).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yolo_mode_parsing() {
        assert_eq!("passive".parse::<YoloMode>().unwrap(), YoloMode::Passive);
        assert_eq!("active".parse::<YoloMode>().unwrap(), YoloMode::Active);
        assert_eq!("aggressive".parse::<YoloMode>().unwrap(), YoloMode::Aggressive);
        assert_eq!("stealth".parse::<YoloMode>().unwrap(), YoloMode::Stealth);
    }

    #[test]
    fn test_yolo_config_default() {
        let config = YoloConfig::default();
        assert!(config.enabled);
        assert_eq!(config.mode, YoloMode::Passive);
        assert_eq!(config.scan_interval_secs, 30);
    }

    #[tokio::test]
    async fn test_daemon_lifecycle() {
        let orchestrator = Arc::new(Orchestrator::new());
        let config = YoloConfig {
            enabled: true,
            mode: YoloMode::Passive,
            scan_interval_secs: 1,
            auto_connect: false,
            max_workers: 5,
            auto_heal: false,
        };

        let daemon = YoloDaemon::new(config, orchestrator);
        assert!(!daemon.is_running());

        daemon.start();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(daemon.is_running());

        daemon.stop();
        assert!(!daemon.is_running());
    }
}
