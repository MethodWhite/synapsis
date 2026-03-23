//! Agent Worker Protocol Implementation
//!
//! This module provides the worker agent protocol for synapsis, including:
//! - WorkerAgent trait for all worker implementations
//! - Built-in workers: ShellWorker, FileWorker, CodeWorker, SearchWorker, GitWorker
//! - External agent connectors: OpenCodeConnector, QwenConnector, ClaudeConnector
//! - Task protocol with JSON serialization
//! - Auto-discovery for available agents

use crate::core::uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;

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
        self.workers.lock().unwrap().insert(id, worker);
    }

    pub fn unregister(&self, id: &str) {
        self.workers.lock().unwrap().remove(id);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn WorkerAgent>> {
        self.workers.lock().unwrap().get(id).cloned()
    }

    pub fn find_by_skill(&self, skill: &str) -> Vec<Arc<dyn WorkerAgent>> {
        self.workers
            .lock()
            .unwrap()
            .values()
            .filter(|w| w.skills().contains(&skill.to_string()))
            .cloned()
            .collect()
    }

    pub fn list(&self) -> Vec<String> {
        self.workers.lock().unwrap().keys().cloned().collect()
    }
}

impl Default for WorkerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ShellWorker {
    id: String,
    skills: Vec<String>,
    max_capacity: usize,
    last_heartbeat: Arc<Mutex<Instant>>,
    antibrick: Option<Arc<crate::core::antibrick::AntiBrickEngine>>,
}

impl Default for ShellWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellWorker {
    pub fn new() -> Self {
        Self {
            id: format!("shell-{}", &Uuid::new_v4().to_hex_string()[..8]),
            skills: vec![
                "shell".to_string(),
                "bash".to_string(),
                "command".to_string(),
                "script".to_string(),
            ],
            max_capacity: 4,
            last_heartbeat: Arc::new(Mutex::new(Instant::now())),
            antibrick: None,
        }
    }

    pub fn with_antibrick(mut self, antibrick: Arc<crate::core::antibrick::AntiBrickEngine>) -> Self {
        self.antibrick = Some(antibrick);
        self
    }

    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.max_capacity = capacity;
        self
    }

    async fn execute_shell_async(&self, command: &str, timeout_secs: u64) -> Result<String, String> {
        // Anti-brick check
        if let Some(ref engine) = self.antibrick {
            let pid = std::process::id(); // Use own PID for logging
            let parts: Vec<String> = command.split_whitespace().map(String::from).collect();
            let (cmd_name, args) = if parts.is_empty() {
                ("", vec![])
            } else {
                (parts[0].as_str(), parts[1..].to_vec())
            };

            if !engine.intercept_command(cmd_name, &args, pid) {
                return Err(format!("BLOCKED by Anti-Brick Protection: Potentially destructive command detected: {}", command));
            }
        }

        let mut child = TokioCommand::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Failed to spawn shell: {}", e))?;

        let timeout = Duration::from_secs(timeout_secs);
        let result = tokio::time::timeout(timeout, async {
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            let mut output = String::new();

            if let Some(mut stdout) = stdout {
                let mut reader = BufReader::new(&mut stdout);
                let _ = reader.read_line(&mut output).await;
            }

            let status = child.wait().await.map_err(|e| format!("Wait failed: {}", e))?;

            if let Some(mut stderr) = stderr {
                let mut err_output = String::new();
                let mut stderr_reader = BufReader::new(&mut stderr);
                let _ = stderr_reader.read_line(&mut err_output).await;
                if !err_output.is_empty() {
                    output.push_str("\nSTDERR: ");
                    output.push_str(&err_output);
                }
            }

            if status.success() {
                Ok(output)
            } else {
                Err(format!("Command failed with status: {}", status))
            }
        })
        .await;

        match result {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                let _ = child.kill().await;
                Err("Command timed out".to_string())
            }
        }
    }
}

impl WorkerAgent for ShellWorker {
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
        if task.task_type != TaskType::Shell {
            return Err(TaskError::unsupported_operation(
                &format!("{:?}", task.task_type),
                &task.id,
            ));
        }

        let start = Instant::now();
        *self.last_heartbeat.lock().unwrap() = Instant::now();

        let command = task
            .payload
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TaskError::new(0x0100, "Missing 'command' in payload", &task.id))?;

        let timeout = task
            .payload
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);

        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let result = rt.block_on(self.execute_shell_async(command, timeout));

        let duration = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => Ok(TaskResult::success(output, duration)),
            Err(e) => Ok(TaskResult::failure(e, duration)),
        }
    }

    fn heartbeat(&self) -> bool {
        *self.last_heartbeat.lock().unwrap() = Instant::now();
        true
    }
}

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
        *self.allowed_dirs.lock().unwrap() = dirs;
        self
    }

    fn is_path_allowed(&self, path: &PathBuf) -> bool {
        let allowed = self.allowed_dirs.lock().unwrap();
        if allowed.is_empty() {
            return true;
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
        *self.last_heartbeat.lock().unwrap() = Instant::now();
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
                    Ok(content) => Ok(TaskResult::success(content, start.elapsed().as_millis() as u64)),
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
                    .ok_or_else(|| TaskError::new(0x0100, "Missing 'content' in payload", &task.id))?;

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
        *self.last_heartbeat.lock().unwrap() = Instant::now();
        true
    }
}

pub struct CodeWorker {
    id: String,
    skills: Vec<String>,
    max_capacity: usize,
    last_heartbeat: Arc<Mutex<Instant>>,
}

impl Default for CodeWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeWorker {
    pub fn new() -> Self {
        Self {
            id: format!("code-{}", &Uuid::new_v4().to_hex_string()[..8]),
            skills: vec![
                "code_analysis".to_string(),
                "refactor".to_string(),
                "lint".to_string(),
                "format".to_string(),
            ],
            max_capacity: 2,
            last_heartbeat: Arc::new(Mutex::new(Instant::now())),
        }
    }

    fn analyze_code(&self, code: &str, language: Option<&str>) -> String {
        let mut analysis = serde_json::json!({
            "lines": code.lines().count(),
            "chars": code.len(),
        });

        let patterns = self.detect_patterns(code);
        analysis["patterns"] = serde_json::json!(patterns);

        if let Some(lang) = language {
            analysis["language"] = serde_json::json!(lang);
        }

        serde_json::to_string_pretty(&analysis).unwrap_or_default()
    }

    fn detect_patterns(&self, code: &str) -> Vec<String> {
        let mut patterns = Vec::new();

        if code.contains("TODO") || code.contains("FIXME") {
            patterns.push("has_todos".to_string());
        }
        if regex::Regex::new(r"\b\w+\s*\(.*\)\s*\{")
            .unwrap()
            .is_match(code)
        {
            patterns.push("has_functions".to_string());
        }
        if code.contains("impl ") {
            patterns.push("has_traits".to_string());
        }
        if code.contains("async fn") || code.contains("await") {
            patterns.push("async_code".to_string());
        }

        patterns
    }

    fn refactor_code(&self, code: &str, style: &str) -> Result<String, String> {
        match style {
            "fmt" => {
                let mut formatted = String::new();
                let mut indent = 0;
                for line in code.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        formatted.push('\n');
                        continue;
                    }
                    if trimmed.ends_with('{') {
                        formatted.push_str(&"    ".repeat(indent));
                        formatted.push_str(trimmed);
                        formatted.push('\n');
                        indent += 1;
                    } else if trimmed.starts_with('}') {
                        indent = indent.saturating_sub(1);
                        formatted.push_str(&"    ".repeat(indent));
                        formatted.push_str(trimmed);
                        formatted.push('\n');
                    } else {
                        formatted.push_str(&"    ".repeat(indent));
                        formatted.push_str(trimmed);
                        formatted.push('\n');
                    }
                }
                Ok(formatted)
            }
            _ => Err(format!("Unknown refactor style: {}", style)),
        }
    }
}

impl WorkerAgent for CodeWorker {
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
        *self.last_heartbeat.lock().unwrap() = Instant::now();
        let start = Instant::now();

        match task.task_type {
            TaskType::CodeAnalysis => {
                let code = task
                    .payload
                    .get("code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TaskError::new(0x0100, "Missing 'code' in payload", &task.id))?;

                let language = task
                    .payload
                    .get("language")
                    .and_then(|v| v.as_str());

                let analysis = self.analyze_code(code, language);
                Ok(TaskResult::success(analysis, start.elapsed().as_millis() as u64))
            }
            TaskType::CodeRefactor => {
                let code = task
                    .payload
                    .get("code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TaskError::new(0x0100, "Missing 'code' in payload", &task.id))?;

                let style = task
                    .payload
                    .get("style")
                    .and_then(|v| v.as_str())
                    .unwrap_or("fmt");

                match self.refactor_code(code, style) {
                    Ok(refactored) => Ok(TaskResult::success(refactored, start.elapsed().as_millis() as u64)),
                    Err(e) => Ok(TaskResult::failure(e, start.elapsed().as_millis() as u64)),
                }
            }
            _ => Err(TaskError::unsupported_operation(
                &format!("{:?}", task.task_type),
                &task.id,
            )),
        }
    }

    fn heartbeat(&self) -> bool {
        *self.last_heartbeat.lock().unwrap() = Instant::now();
        true
    }
}

pub struct SearchWorker {
    id: String,
    skills: Vec<String>,
    max_capacity: usize,
    last_heartbeat: Arc<Mutex<Instant>>,
}

impl Default for SearchWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchWorker {
    pub fn new() -> Self {
        Self {
            id: format!("search-{}", &Uuid::new_v4().to_hex_string()[..8]),
            skills: vec![
                "search".to_string(),
                "web_search".to_string(),
                "code_search".to_string(),
                "grep".to_string(),
            ],
            max_capacity: 4,
            last_heartbeat: Arc::new(Mutex::new(Instant::now())),
        }
    }

    async fn web_search_async(&self, query: &str) -> Result<String, String> {
        let output = TokioCommand::new("curl")
            .args(["-s", "https://api.duckduckgo.com/", "-d", &format!("q={}", query), "-d", "format=json"])
            .output()
            .await
            .map_err(|e| format!("curl failed: {}", e))?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|e| format!("Invalid UTF-8: {}", e))
        } else {
            Err(format!("Search failed: {:?}", output.status))
        }
    }

    fn code_search(&self, pattern: &str, path: &str, file_filter: Option<&str>) -> Result<String, String> {
        let mut args = vec!["-r".to_string(), pattern.to_string(), path.to_string()];

        if let Some(filter) = file_filter {
            args.push("--include".to_string());
            args.push(filter.to_string());
        }

        let output = std::process::Command::new("rg")
            .args(&args)
            .output()
            .map_err(|e| format!("ripgrep failed: {}", e))?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|e| format!("Invalid UTF-8: {}", e))
        } else if output.status.code() == Some(1) {
            Ok(String::from("No matches found"))
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

impl WorkerAgent for SearchWorker {
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
        *self.last_heartbeat.lock().unwrap() = Instant::now();
        let start = Instant::now();

        let search_type = task
            .payload
            .get("search_type")
            .and_then(|v| v.as_str())
            .unwrap_or("code");

        match search_type {
            "web" => {
                let query = task
                    .payload
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TaskError::new(0x0100, "Missing 'query' in payload", &task.id))?;

                let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
                match rt.block_on(self.web_search_async(query)) {
                    Ok(results) => Ok(TaskResult::success(results, start.elapsed().as_millis() as u64)),
                    Err(e) => Ok(TaskResult::failure(e, start.elapsed().as_millis() as u64)),
                }
            }
            "code" => {
                let pattern = task
                    .payload
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| TaskError::new(0x0100, "Missing 'pattern' in payload", &task.id))?;

                let path = task
                    .payload
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");

                let file_filter = task
                    .payload
                    .get("file_filter")
                    .and_then(|v| v.as_str());

                match self.code_search(pattern, path, file_filter) {
                    Ok(results) => Ok(TaskResult::success(results, start.elapsed().as_millis() as u64)),
                    Err(e) => Ok(TaskResult::failure(e, start.elapsed().as_millis() as u64)),
                }
            }
            _ => Err(TaskError::unsupported_operation(
                &format!("search type: {}", search_type),
                &task.id,
            )),
        }
    }

    fn heartbeat(&self) -> bool {
        *self.last_heartbeat.lock().unwrap() = Instant::now();
        true
    }
}

pub struct GitWorker {
    id: String,
    skills: Vec<String>,
    max_capacity: usize,
    last_heartbeat: Arc<Mutex<Instant>>,
}

impl Default for GitWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl GitWorker {
    pub fn new() -> Self {
        Self {
            id: format!("git-{}", &Uuid::new_v4().to_hex_string()[..8]),
            skills: vec![
                "git".to_string(),
                "version_control".to_string(),
                "vcs".to_string(),
            ],
            max_capacity: 2,
            last_heartbeat: Arc::new(Mutex::new(Instant::now())),
        }
    }

    async fn execute_git_async(&self, args: Vec<&str>, cwd: Option<PathBuf>) -> Result<String, String> {
        let mut cmd = TokioCommand::new("git");
        for arg in &args {
            cmd.arg(*arg);
        }
        if let Some(ref dir) = cwd {
            cmd.current_dir(dir);
        }

        let output = cmd
            .output()
            .await
            .map_err(|e| format!("git command failed: {}", e))?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|e| format!("Invalid UTF-8: {}", e))
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

impl WorkerAgent for GitWorker {
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
        if task.task_type != TaskType::Git {
            return Err(TaskError::unsupported_operation(
                &format!("{:?}", task.task_type),
                &task.id,
            ));
        }

        *self.last_heartbeat.lock().unwrap() = Instant::now();
        let start = Instant::now();

        let command = task
            .payload
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TaskError::new(0x0100, "Missing 'command' in payload", &task.id))?;

        let cwd = task
            .payload
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);

        let git_args: Vec<&str> = command.split_whitespace().collect();
        if git_args.is_empty() {
            return Err(TaskError::new(0x0100, "Empty git command", &task.id));
        }

        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        match rt.block_on(self.execute_git_async(git_args, cwd)) {
            Ok(output) => Ok(TaskResult::success(output, start.elapsed().as_millis() as u64)),
            Err(e) => Ok(TaskResult::failure(e, start.elapsed().as_millis() as u64)),
        }
    }

    fn heartbeat(&self) -> bool {
        *self.last_heartbeat.lock().unwrap() = Instant::now();
        true
    }
}

pub struct ExternalAgentConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub timeout_secs: u64,
}

impl Default for ExternalAgentConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            timeout_secs: 300,
        }
    }
}

pub trait ExternalAgentConnector: Send + Sync {
    fn agent_id(&self) -> &str;
    fn agent_type(&self) -> &str;
    fn is_running(&self) -> bool;
    fn spawn(&mut self) -> Result<(), TaskError>;
    fn send_task(&mut self, task: Task) -> Result<(), TaskError>;
    fn poll_result(&mut self) -> Option<TaskResult>;
    fn shutdown(&mut self) -> Result<(), TaskError>;
}

pub struct OpenCodeConnector {
    config: ExternalAgentConfig,
    process: Option<Child>,
    pending_tasks: Arc<Mutex<HashMap<String, Task>>>,
    running: Arc<Mutex<bool>>,
}

impl OpenCodeConnector {
    pub fn new(config: ExternalAgentConfig) -> Self {
        Self {
            config,
            process: None,
            pending_tasks: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(ExternalAgentConfig {
            command: "opencode".to_string(),
            args: vec!["--agent".to_string(), "--mode".to_string(), "pipe".to_string()],
            env: HashMap::new(),
            timeout_secs: 300,
        })
    }
}

impl ExternalAgentConnector for OpenCodeConnector {
    fn agent_id(&self) -> &str {
        "opencode"
    }

    fn agent_type(&self) -> &str {
        "opencode"
    }

    fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    fn spawn(&mut self) -> Result<(), TaskError> {
        if self.is_running() {
            return Err(TaskError::new(0x0200, "Agent already running", "opencode"));
        }

        let mut cmd = Command::new(&self.config.command);
        for arg in &self.config.args {
            cmd.arg(arg);
        }
        for (key, val) in &self.config.env {
            cmd.env(key, val);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| TaskError::execution_failed(&format!("Failed to spawn opencode: {}", e), "opencode"))?;

        self.process = Some(child);
        *self.running.lock().unwrap() = true;
        Ok(())
    }

    fn send_task(&mut self, task: Task) -> Result<(), TaskError> {
        if !self.is_running() {
            return Err(TaskError::new(0x0201, "Agent not running", "opencode"));
        }

        let task_json = serde_json::to_string(&task)
            .map_err(|e| TaskError::new(0x0202, &format!("Failed to serialize task: {}", e), "opencode"))?;

        self.pending_tasks.lock().unwrap().insert(task.id.clone(), task);

        if let Some(ref mut child) = self.process {
            if let Some(ref mut stdin) = child.stdin {
                stdin
                    .write_all(task_json.as_bytes())
                    .map_err(|e| TaskError::execution_failed(&format!("Failed to send task: {}", e), "opencode"))?;
            }
        }

        Ok(())
    }

    fn poll_result(&mut self) -> Option<TaskResult> {
        if !self.is_running() {
            return None;
        }

        if let Some(ref mut child) = self.process {
            if let Ok(Some(status)) = child.try_wait() {
                if status.success() {
                    *self.running.lock().unwrap() = false;
                }
            }
        }

        None
    }

    fn shutdown(&mut self) -> Result<(), TaskError> {
        if let Some(ref mut child) = self.process {
            child
                .kill()
                .map_err(|e| TaskError::execution_failed(&format!("Failed to kill agent: {}", e), "opencode"))?;
        }

        self.process = None;
        *self.running.lock().unwrap() = false;
        Ok(())
    }
}

pub struct QwenConnector {
    config: ExternalAgentConfig,
    process: Option<Child>,
    pending_tasks: Arc<Mutex<HashMap<String, Task>>>,
    running: Arc<Mutex<bool>>,
}

impl QwenConnector {
    pub fn new(config: ExternalAgentConfig) -> Self {
        Self {
            config,
            process: None,
            pending_tasks: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(ExternalAgentConfig {
            command: "qwen".to_string(),
            args: vec!["--agent".to_string(), "--pipe".to_string()],
            env: HashMap::new(),
            timeout_secs: 300,
        })
    }
}

impl ExternalAgentConnector for QwenConnector {
    fn agent_id(&self) -> &str {
        "qwen"
    }

    fn agent_type(&self) -> &str {
        "qwen"
    }

    fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    fn spawn(&mut self) -> Result<(), TaskError> {
        if self.is_running() {
            return Err(TaskError::new(0x0200, "Agent already running", "qwen"));
        }

        let mut cmd = Command::new(&self.config.command);
        for arg in &self.config.args {
            cmd.arg(arg);
        }
        for (key, val) in &self.config.env {
            cmd.env(key, val);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| TaskError::execution_failed(&format!("Failed to spawn qwen: {}", e), "qwen"))?;

        self.process = Some(child);
        *self.running.lock().unwrap() = true;
        Ok(())
    }

    fn send_task(&mut self, task: Task) -> Result<(), TaskError> {
        if !self.is_running() {
            return Err(TaskError::new(0x0201, "Agent not running", "qwen"));
        }

        let task_json = serde_json::to_string(&task)
            .map_err(|e| TaskError::new(0x0202, &format!("Failed to serialize task: {}", e), "qwen"))?;

        self.pending_tasks.lock().unwrap().insert(task.id.clone(), task);

        if let Some(ref mut child) = self.process {
            if let Some(ref mut stdin) = child.stdin {
                stdin
                    .write_all(task_json.as_bytes())
                    .map_err(|e| TaskError::execution_failed(&format!("Failed to send task: {}", e), "qwen"))?;
            }
        }

        Ok(())
    }

    fn poll_result(&mut self) -> Option<TaskResult> {
        if !self.is_running() {
            return None;
        }

        if let Some(ref mut child) = self.process {
            if let Ok(Some(status)) = child.try_wait() {
                if status.success() {
                    *self.running.lock().unwrap() = false;
                }
            }
        }

        None
    }

    fn shutdown(&mut self) -> Result<(), TaskError> {
        if let Some(ref mut child) = self.process {
            child
                .kill()
                .map_err(|e| TaskError::execution_failed(&format!("Failed to kill agent: {}", e), "qwen"))?;
        }

        self.process = None;
        *self.running.lock().unwrap() = false;
        Ok(())
    }
}

pub struct ClaudeConnector {
    config: ExternalAgentConfig,
    process: Option<Child>,
    pending_tasks: Arc<Mutex<HashMap<String, Task>>>,
    running: Arc<Mutex<bool>>,
}

impl ClaudeConnector {
    pub fn new(config: ExternalAgentConfig) -> Self {
        Self {
            config,
            process: None,
            pending_tasks: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(ExternalAgentConfig {
            command: "claude".to_string(),
            args: vec!["--agent".to_string(), "--pipe".to_string()],
            env: HashMap::new(),
            timeout_secs: 300,
        })
    }
}

impl ExternalAgentConnector for ClaudeConnector {
    fn agent_id(&self) -> &str {
        "claude"
    }

    fn agent_type(&self) -> &str {
        "claude"
    }

    fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    fn spawn(&mut self) -> Result<(), TaskError> {
        if self.is_running() {
            return Err(TaskError::new(0x0200, "Agent already running", "claude"));
        }

        let mut cmd = Command::new(&self.config.command);
        for arg in &self.config.args {
            cmd.arg(arg);
        }
        for (key, val) in &self.config.env {
            cmd.env(key, val);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| TaskError::execution_failed(&format!("Failed to spawn claude: {}", e), "claude"))?;

        self.process = Some(child);
        *self.running.lock().unwrap() = true;
        Ok(())
    }

    fn send_task(&mut self, task: Task) -> Result<(), TaskError> {
        if !self.is_running() {
            return Err(TaskError::new(0x0201, "Agent not running", "claude"));
        }

        let task_json = serde_json::to_string(&task)
            .map_err(|e| TaskError::new(0x0202, &format!("Failed to serialize task: {}", e), "claude"))?;

        self.pending_tasks.lock().unwrap().insert(task.id.clone(), task);

        if let Some(ref mut child) = self.process {
            if let Some(ref mut stdin) = child.stdin {
                stdin
                    .write_all(task_json.as_bytes())
                    .map_err(|e| TaskError::execution_failed(&format!("Failed to send task: {}", e), "claude"))?;
            }
        }

        Ok(())
    }

    fn poll_result(&mut self) -> Option<TaskResult> {
        if !self.is_running() {
            return None;
        }

        if let Some(ref mut child) = self.process {
            if let Ok(Some(status)) = child.try_wait() {
                if status.success() {
                    *self.running.lock().unwrap() = false;
                }
            }
        }

        None
    }

    fn shutdown(&mut self) -> Result<(), TaskError> {
        if let Some(ref mut child) = self.process {
            child
                .kill()
                .map_err(|e| TaskError::execution_failed(&format!("Failed to kill agent: {}", e), "claude"))?;
        }

        self.process = None;
        *self.running.lock().unwrap() = false;
        Ok(())
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

        let opencode_check = std::process::Command::new("which").arg("opencode").output();
        if let Ok(output) = opencode_check {
            if output.status.success() {
                agents.push(AvailableAgent {
                    name: "opencode".to_string(),
                    path: "opencode".to_string(),
                    connector_type: "opencode".to_string(),
                });
            }
        }

        let qwen_check = std::process::Command::new("which").arg("qwen").output();
        if let Ok(output) = qwen_check {
            if output.status.success() {
                agents.push(AvailableAgent {
                    name: "qwen".to_string(),
                    path: "qwen".to_string(),
                    connector_type: "qwen".to_string(),
                });
            }
        }

        let claude_check = std::process::Command::new("which").arg("claude").output();
        if let Ok(output) = claude_check {
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
        let shell_worker = Arc::new(ShellWorker::new());
        self.registry.register(shell_worker);

        let file_worker = Arc::new(FileWorker::new());
        self.registry.register(file_worker);

        let code_worker = Arc::new(CodeWorker::new());
        self.registry.register(code_worker);

        let search_worker = Arc::new(SearchWorker::new());
        self.registry.register(search_worker);

        let git_worker = Arc::new(GitWorker::new());
        self.registry.register(git_worker);
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

pub struct WorkerOrchestrator {
    registry: WorkerRegistry,
    external_connectors: HashMap<String, Box<dyn ExternalAgentConnector>>,
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

    pub fn register_connector(&mut self, connector: Box<dyn ExternalAgentConnector>) {
        let id = connector.agent_id().to_string();
        self.external_connectors.insert(id, connector);
    }

    pub fn find_worker_for_task(&self, task: &Task) -> Option<Arc<dyn WorkerAgent>> {
        for skill in &task.skills_required {
            if let Some(workers) = Some(self.registry.find_by_skill(skill)) {
                if let Some(worker) = workers.first() {
                    return Some(worker.clone());
                }
            }
        }
        None
    }

    pub fn execute_task(&self, task: Task) -> Result<TaskResult, TaskError> {
        if let Some(worker) = self.find_worker_for_task(&task) {
            worker.execute_task(&task)
        } else {
            Err(TaskError::unsupported_operation("No suitable worker found", &task.id))
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

// ============================================================================
// STALE SESSION CLEANUP JOB
// ============================================================================

use tokio::time::interval;

/// Automatic cleanup job for stale agent sessions
/// Runs every 3600 seconds (1 hour) and removes sessions with:
/// - is_active = 0
/// - last_heartbeat > 3600 seconds ago
pub struct SessionCleanupJob {
    db: Arc<crate::infrastructure::database::Database>,
    interval_secs: u64,
}

impl SessionCleanupJob {
    pub fn new(db: Arc<crate::infrastructure::database::Database>) -> Self {
        Self {
            db,
            interval_secs: 3600, // 1 hour
        }
    }

    pub fn with_interval(db: Arc<crate::infrastructure::database::Database>, interval_secs: u64) -> Self {
        Self { db, interval_secs }
    }

    /// Start the cleanup job (runs in background)
    pub async fn start(&self) {
        let mut interval_timer = interval(std::time::Duration::from_secs(self.interval_secs));
        
        eprintln!(
            "[SessionCleanup] Started - running every {} seconds",
            self.interval_secs
        );

        loop {
            interval_timer.tick().await;
            self.run_cleanup();
        }
    }

    /// Run a single cleanup cycle
    pub fn run_cleanup(&self) -> usize {
        let threshold = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64) - 3600;

        match self.db.cleanup_stale_sessions(threshold) {
            Ok(count) => {
                if count > 0 {
                    eprintln!(
                        "[SessionCleanup] Cleaned {} stale sessions (threshold: {}s)",
                        count, threshold
                    );
                }
                count
            }
            Err(e) => {
                eprintln!("[SessionCleanup] Error: {}", e);
                0
            }
        }
    }
}

/// Start cleanup job in background thread (non-async version)
pub fn start_cleanup_job_background(db: Arc<crate::infrastructure::database::Database>) {
    let cleanup = SessionCleanupJob::new(db);
    
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(cleanup.start());
    });
}
