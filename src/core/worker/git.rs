use super::{Task, TaskError, TaskResult, TaskType, WorkerAgent};
use crate::core::lock_utils::*;
use crate::core::uuid::Uuid;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tokio::process::Command as TokioCommand;

const ALLOWED_GIT_SUBCOMMANDS: &[&str] = &[
    "log",
    "diff",
    "status",
    "show",
    "branch",
    "tag",
    "describe",
    "rev-parse",
    "rev-list",
    "rev-count",
    "shortlog",
];
const BLOCKED_GIT_FLAGS: &[&str] = &["-c", "--config-env", "--config"];

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

    async fn execute_git_async(
        &self,
        args: Vec<&str>,
        cwd: Option<PathBuf>,
    ) -> Result<String, String> {
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
            String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8: {}", e))
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

        *self.last_heartbeat.lock_safe() = Instant::now();
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

        // Security: block dangerous git flags
        for arg in &git_args {
            if BLOCKED_GIT_FLAGS.contains(arg) {
                return Err(TaskError::new(
                    0x0100,
                    &format!(
                        "Blocked git flag: {}. Only safe subcommands are allowed.",
                        arg
                    ),
                    &task.id,
                ));
            }
        }

        // Security: only allow safe subcommands
        let subcmd = git_args[0];
        if !ALLOWED_GIT_SUBCOMMANDS.contains(&subcmd) {
            return Err(TaskError::new(
                0x0100,
                &format!(
                    "Git subcommand '{}' is not allowed. Allowed: {:?}",
                    subcmd, ALLOWED_GIT_SUBCOMMANDS
                ),
                &task.id,
            ));
        }

        static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        let rt = RT.get_or_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build git runtime")
        });
        match rt.block_on(self.execute_git_async(git_args, cwd)) {
            Ok(output) => Ok(TaskResult::success(
                output,
                start.elapsed().as_millis() as u64,
            )),
            Err(e) => Ok(TaskResult::failure(e, start.elapsed().as_millis() as u64)),
        }
    }

    fn heartbeat(&self) -> bool {
        *self.last_heartbeat.lock_safe() = Instant::now();
        true
    }
}
