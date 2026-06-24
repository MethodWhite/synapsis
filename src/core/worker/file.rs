use super::{Task, TaskError, TaskResult, TaskType, WorkerAgent};
use crate::core::lock_utils::*;
use crate::core::uuid::Uuid;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct FileWorker {
    id: String,
    skills: Vec<String>,
    max_capacity: usize,
    allowed_dirs: Arc<Mutex<Vec<PathBuf>>>,
    last_heartbeat: Arc<Mutex<Instant>>,
}

impl Default for FileWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWorker {
    pub fn new() -> Self {
        Self {
            id: format!("file-{}", &Uuid::new_v4().to_hex_string()[..8]),
            skills: vec![
                "file_read".to_string(),
                "file_write".to_string(),
                "filesystem".to_string(),
            ],
            max_capacity: 8,
            allowed_dirs: Arc::new(Mutex::new(Vec::new())),
            last_heartbeat: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn with_allowed_dirs(self, dirs: Vec<PathBuf>) -> Self {
        *self.allowed_dirs.lock_safe() = dirs;
        self
    }

    fn is_path_allowed(&self, path: &Path) -> bool {
        let allowed = self.allowed_dirs.lock_safe();
        if allowed.is_empty() {
            return false;
        }
        allowed.iter().any(|d| path.starts_with(d))
    }
}

impl WorkerAgent for FileWorker {
    fn id(&self) -> &str {
        &self.id
    }

    fn skills(&self) -> Vec<String> {
        self.skills.clone()
    }

    fn capacity(&self) -> usize {
        self.max_capacity
    }

    fn execute_task(&self, task: &Task) -> Result<TaskResult, TaskError> {
        *self.last_heartbeat.lock_safe() = Instant::now();
        let start = Instant::now();

        match task.task_type {
            TaskType::FileRead => {
                let path = task
                    .payload
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TaskError::new(0x0100, "Missing 'path' in payload", &task.id))?;

                let path_buf = PathBuf::from(path);
                if !self.is_path_allowed(&path_buf) {
                    return Err(TaskError::permission_denied(path, &task.id));
                }

                match std::fs::read_to_string(path) {
                    Ok(content) => Ok(TaskResult::success(
                        content,
                        start.elapsed().as_millis() as u64,
                    )),
                    Err(e) => Err(TaskError::execution_failed(&e.to_string(), &task.id)),
                }
            }
            TaskType::FileWrite => {
                let path = task
                    .payload
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TaskError::new(0x0100, "Missing 'path' in payload", &task.id))?;

                let content = task
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        TaskError::new(0x0100, "Missing 'content' in payload", &task.id)
                    })?;

                let path_buf = PathBuf::from(path);
                if !self.is_path_allowed(&path_buf) {
                    return Err(TaskError::permission_denied(path, &task.id));
                }

                match std::fs::write(path, content) {
                    Ok(_) => Ok(TaskResult::success(
                        format!("Wrote {} bytes to {}", content.len(), path),
                        start.elapsed().as_millis() as u64,
                    )),
                    Err(e) => Err(TaskError::execution_failed(&e.to_string(), &task.id)),
                }
            }
            _ => Err(TaskError::unsupported_operation(
                &format!("{:?}", task.task_type),
                &task.id,
            )),
        }
    }

    fn heartbeat(&self) -> bool {
        *self.last_heartbeat.lock_safe() = Instant::now();
        true
    }
}
