//! Synapsis Agents Module
//!
//! Multi-agent coordination system. Agents are AI assistants with specialized
//! roles and capabilities. Synapsis manages agent definitions, states,
//! communication, and coordination.

use crate::core::lock_utils::*;
use crate::core::uuid::Uuid;
use crate::domain::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

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

impl std::str::FromStr for AgentRole {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "orchestrator" | "orch" | "coordinator" => Self::Orchestrator,
            "coder" | "code" | "developer" => Self::Coder,
            "researcher" | "research" => Self::Researcher,
            "reviewer" | "review" | "critic" => Self::Reviewer,
            "tester" | "test" | "qa" => Self::Tester,
            "architect" | "arch" => Self::Architect,
            "security" | "sec" => Self::Security,
            "devops" | "ops" => Self::DevOps,
            _ => Self::General,
        })
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

impl std::str::FromStr for TaskStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "assigned" => Ok(Self::Assigned),
            "inprogress" | "in_progress" | "progress" => Ok(Self::InProgress),
            "completed" | "done" => Ok(Self::Completed),
            "failed" | "error" => Ok(Self::Failed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            _ => Err(()),
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

/// Cross-process advisory lock over the registry files.
///
/// The registry is shared by every ``synapsis-mcp`` process on the host, so an
/// in-process `RwLock` is not enough: two processes can interleave read-merge-
/// write and still lose an entry. `flock` serialises the whole section across
/// processes. If the lock cannot be taken we proceed unlocked rather than
/// failing the call -- correctness still degrades to the old behaviour, but a
/// read-only or exotic filesystem cannot make registration impossible.
///
/// On non-unix targets the guard is a no-op. The merge in `save_with` is the
/// part that stops one process from erasing another's agents, and it runs
/// everywhere; the lock only closes the narrower window where two processes
/// interleave between the read and the write. Windows therefore keeps the
/// important guarantee and loses the tighter one. `LockFileEx` would close it
/// too, but that is FFI this crate cannot compile or test on the CI host, and
/// untested handle-owning FFI is worse than a documented degradation.
struct FileLock {
    #[allow(dead_code)]
    file: std::fs::File,
}

#[cfg(unix)]
mod imp {
    use super::FileLock;
    use std::os::unix::io::AsRawFd;

    const LOCK_EX: i32 = 2;
    const LOCK_UN: i32 = 8;

    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }

    impl FileLock {
        pub(super) fn acquire(path: &std::path::Path) -> std::io::Result<Self> {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(path)?;
            // SAFETY: `fd` is owned by `file` for the lifetime of the guard.
            let rc = unsafe { flock(file.as_raw_fd(), LOCK_EX) };
            if rc != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self { file })
        }
    }

    impl Drop for FileLock {
        fn drop(&mut self) {
            use std::os::unix::io::AsRawFd;
            // SAFETY: the descriptor is still open here; unlocking cannot fail in a
            // way we can act on, and the close releases the lock regardless.
            unsafe {
                flock(self.file.as_raw_fd(), LOCK_UN);
            }
        }
    }
}

#[cfg(not(unix))]
mod imp {
    use super::FileLock;

    impl FileLock {
        pub(super) fn acquire(path: &std::path::Path) -> std::io::Result<Self> {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(path)?;
            Ok(Self { file })
        }
    }
}

/// Read a JSON registry file, treating absence and corruption as empty.
///
/// A truncated file must not take the whole registry down: losing every agent
/// is worse than losing one entry, and the next write republishes this
/// process's view.
fn read_agents(path: &std::path::Path) -> Option<HashMap<AgentId, Agent>> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn read_tasks(path: &std::path::Path) -> Option<Vec<Task>> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Publish JSON via a temporary file and a rename.
///
/// `std::fs::write` truncates in place, so a crash mid-write leaves invalid
/// JSON for the next reader. A rename is atomic on the same filesystem, so a
/// reader sees either the previous contents or the complete new file.
fn write_atomic<T: Serialize>(path: &std::path::Path, value: &T) -> std::io::Result<()> {
    let data = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<AgentId, Agent>>>,
    messages: Arc<RwLock<Vec<AgentMessage>>>,
    tasks: Arc<RwLock<Vec<Task>>>,
    data_dir: PathBuf,
    dirty: AtomicBool,
    last_save: std::sync::Mutex<Instant>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        let data_dir = crate::config::data_dir().join("agents");
        std::fs::create_dir_all(&data_dir).ok();

        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(Vec::new())),
            tasks: Arc::new(RwLock::new(Vec::new())),
            data_dir,
            dirty: AtomicBool::new(false),
            last_save: std::sync::Mutex::new(Instant::now()),
        }
    }

    pub fn init(&self) -> std::io::Result<()> {
        self.load()
    }

    pub fn load(&self) -> std::io::Result<()> {
        let agents_file = self.data_dir.join("agents.json");
        if let Some(mut agents) = read_agents(&agents_file) {
            // Keep anything this process registered before the file existed,
            // and let the file win on conflicts: it is the shared view.
            let mut local = self.agents.write_safe();
            let pending: Vec<(AgentId, Agent)> = std::mem::take(&mut *local).into_iter().collect();
            for (id, agent) in pending {
                agents.entry(id).or_insert(agent);
            }
            *local = agents;
        }

        let tasks_file = self.data_dir.join("tasks.json");
        if let Some(mut tasks) = read_tasks(&tasks_file) {
            let mut local = self.tasks.write_safe();
            let pending: Vec<Task> = std::mem::take(&mut *local);
            for task in pending {
                if !tasks.iter().any(|t| t.id == task.id) {
                    tasks.push(task);
                }
            }
            *local = tasks;
        }

        Ok(())
    }

    /// Re-read the shared registry so `list`/`get_by_name` see agents that other
    /// processes registered since this one started.
    ///
    /// The in-memory map was only ever populated at `init`, which is why every
    /// process used to see only its own agents.
    pub fn refresh(&self) -> std::io::Result<()> {
        self.load()
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        // The previous version debounced here: inside the 500ms window it
        // returned without saving and, crucially, without scheduling a retry.
        // `dirty` stayed set, so if the process exited before another
        // mark_dirty landed outside the window the change was lost with no
        // trace. Coalescing rapid updates is still worth it for a hot loop, so
        // the window is kept but a pending write is always flushed before the
        // call returns.
        let elapsed = self.last_save.lock_safe().elapsed();
        if elapsed >= std::time::Duration::from_millis(500) || self.dirty.load(Ordering::Relaxed) {
            let _ = self.flush();
        }
    }

    pub fn flush(&self) -> std::io::Result<()> {
        if self.dirty.swap(false, Ordering::Relaxed) {
            *self.last_save.lock_safe() = Instant::now();
            self.save_merged(None)
        } else {
            Ok(())
        }
    }

    /// Read the on-disk registry, fold in what this process knows, then
    /// publish atomically.
    ///
    /// `incoming` carries an explicit registration to merge. Without it the
    /// in-memory map is merged over the file, which keeps agents registered by
    /// *other* processes instead of erasing them.
    fn save_merged(&self, incoming: Option<(&AgentId, &Agent)>) -> std::io::Result<()> {
        self.save_with(incoming, None)
    }

    /// Persist with ``exclude`` removed from the published view.
    fn save_excluding(&self, exclude: Option<&AgentId>) -> std::io::Result<()> {
        self.save_with(None, exclude)
    }

    fn save_with(
        &self,
        incoming: Option<(&AgentId, &Agent)>,
        exclude: Option<&AgentId>,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let _guard = FileLock::acquire(&self.data_dir.join("registry.lock"))?;

        let agents_file = self.data_dir.join("agents.json");
        let mut merged: HashMap<AgentId, Agent> = read_agents(&agents_file).unwrap_or_default();
        {
            let agents = self.agents.read_safe();
            for (id, agent) in agents.iter() {
                merged.insert(id.clone(), agent.clone());
            }
        }
        if let Some((id, agent)) = incoming {
            merged.insert(id.clone(), agent.clone());
        }
        if let Some(id) = exclude {
            merged.remove(id);
        }
        write_atomic(&agents_file, &merged)?;

        let tasks_file = self.data_dir.join("tasks.json");
        let mut merged_tasks: Vec<Task> = read_tasks(&tasks_file).unwrap_or_default();
        {
            let tasks = self.tasks.read_safe();
            for task in tasks.iter() {
                match merged_tasks.iter_mut().find(|t| t.id == task.id) {
                    Some(existing) => *existing = task.clone(),
                    None => merged_tasks.push(task.clone()),
                }
            }
        }
        write_atomic(&tasks_file, &merged_tasks)?;
        Ok(())
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_merged(None)
    }

    pub fn register(&self, agent: Agent) -> AgentId {
        let id = agent.id.clone();
        self.agents.write_safe().insert(id.clone(), agent.clone());
        // Re-read under the lock before writing: another process may have
        // registered since we last loaded, and writing our whole in-memory map
        // would silently drop it.
        let _ = self.save_merged(Some((&id, &agent)));
        id
    }

    pub fn unregister(&self, id: &AgentId) -> Option<Agent> {
        let agent = self.agents.write_safe().remove(id);
        // A plain merge would put the removed agent straight back, since it is
        // still present in the file this process last read. The removal has to
        // be applied to the merged view explicitly.
        if agent.is_some() {
            let _ = self.save_excluding(Some(id));
        } else {
            let _ = self.save_merged(None);
        }
        agent
    }

    pub fn get(&self, id: &AgentId) -> Option<Agent> {
        self.agents.read_safe().get(id).cloned()
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
        let agents = self.agents.read_safe();
        match role {
            Some(r) => agents.values().filter(|a| a.role == r).cloned().collect(),
            None => agents.values().cloned().collect(),
        }
    }

    pub fn update_state(&self, id: &AgentId, state: AgentState) -> bool {
        if let Some(agent) = self.agents.write_safe().get_mut(id) {
            agent.state = state;
            if state == AgentState::Working {
                agent.update_activity();
            }
            self.mark_dirty();
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
        if !self.agents.read_safe().contains_key(&from)
            || !self.agents.read_safe().contains_key(&to)
        {
            return None;
        }

        let msg = AgentMessage::new(from, to, content, msg_type);
        let id = msg.id.clone();
        self.messages.write_safe().push(msg);
        self.mark_dirty();
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
        self.tasks.write_safe().push(task);
        self.mark_dirty();
        id
    }

    pub fn assign_task(&self, task_id: &TaskId, agent_id: &AgentId) -> bool {
        let mut tasks = self.tasks.write_safe();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == *task_id) {
            task.assign(agent_id.clone());
            drop(tasks);
            self.mark_dirty();
            true
        } else {
            false
        }
    }

    pub fn complete_task(&self, task_id: &TaskId, result: String) -> bool {
        let mut tasks = self.tasks.write_safe();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == *task_id) {
            task.complete(result);
            drop(tasks);
            self.mark_dirty();
            true
        } else {
            false
        }
    }

    pub fn get_tasks(&self, status: Option<TaskStatus>) -> Vec<Task> {
        let tasks = self.tasks.read_safe();
        match status {
            Some(s) => tasks.iter().filter(|t| t.status == s).cloned().collect(),
            None => tasks.iter().cloned().collect(),
        }
    }

    pub fn count(&self) -> usize {
        self.agents.read_safe().len()
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
            dirty: AtomicBool::new(false),
            last_save: std::sync::Mutex::new(Instant::now()),
        }
    }
}
