use super::{Task, TaskError, TaskResult, TaskType, WorkerAgent};
use crate::core::lock_utils::*;
use crate::core::uuid::Uuid;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::process::Command as TokioCommand;

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

    pub fn with_antibrick(
        mut self,
        antibrick: Arc<crate::core::antibrick::AntiBrickEngine>,
    ) -> Self {
        self.antibrick = Some(antibrick);
        self
    }

    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.max_capacity = capacity;
        self
    }

    fn has_dangerous_metachars(cmd: &str) -> bool {
        let dangerous = [';', '|', '`', '$', '>', '<', '&', '\n', '\r'];
        cmd.contains(&dangerous[..])
    }

    async fn execute_shell_async(
        &self,
        command: &str,
        timeout_secs: u64,
    ) -> Result<String, String> {
        if Self::has_dangerous_metachars(command) {
            return Err(format!(
                "BLOCKED: Dangerous shell metacharacters in command: {}",
                command
            ));
        }

        if let Some(ref engine) = self.antibrick {
            let pid = std::process::id();
            let parts: Vec<String> = command.split_whitespace().map(String::from).collect();
            let (cmd_name, args) = if parts.is_empty() {
                ("", vec![])
            } else {
                (parts[0].as_str(), parts[1..].to_vec())
            };

            if !engine.intercept_command(cmd_name, &args, pid) {
                return Err(format!("BLOCKED by Anti-Brick Protection: {}", command));
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

        let timeout = std::time::Duration::from_secs(timeout_secs);
        let result = tokio::time::timeout(timeout, async {
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            let mut output = String::new();

            if let Some(mut stdout) = stdout {
                let mut reader = tokio::io::BufReader::new(&mut stdout);
                let _ = reader.read_to_string(&mut output).await;
            }

            let status = child
                .wait()
                .await
                .map_err(|e| format!("Wait failed: {}", e))?;

            if let Some(mut stderr) = stderr {
                let mut err_output = String::new();
                let mut stderr_reader = tokio::io::BufReader::new(&mut stderr);
                let _ = stderr_reader.read_to_string(&mut err_output).await;
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
        *self.last_heartbeat.lock_safe() = Instant::now();

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

        static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
        let rt = RT.get_or_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build shell runtime")
        });
        let result = rt.block_on(self.execute_shell_async(command, timeout));

        let duration = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => Ok(TaskResult::success(output, duration)),
            Err(e) => Ok(TaskResult::failure(e, duration)),
        }
    }

    fn heartbeat(&self) -> bool {
        *self.last_heartbeat.lock_safe() = Instant::now();
        true
    }
}
