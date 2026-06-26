use super::{Task, TaskError, TaskResult, WorkerAgent};
use crate::core::lock_utils::*;
use crate::core::uuid::Uuid;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::process::Command as TokioCommand;

fn urlencode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push('+'),
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
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
            .args([
                "-s",
                "https://api.duckduckgo.com/",
                "-d",
                &format!("q={}", urlencode(query)),
                "-d",
                "format=json",
            ])
            .output()
            .await
            .map_err(|e| format!("curl failed: {}", e))?;

        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8: {}", e))
        } else {
            Err(format!("Search failed: {:?}", output.status))
        }
    }

    fn code_search(
        &self,
        pattern: &str,
        path: &str,
        file_filter: Option<&str>,
    ) -> Result<String, String> {
        let mut args = vec!["-r".to_string(), pattern.to_string()];

        if let Some(filter) = file_filter {
            args.push("--include".to_string());
            args.push(filter.to_string());
        }

        args.push("--".to_string());
        args.push(path.to_string());

        let output = std::process::Command::new("rg")
            .args(&args)
            .output()
            .map_err(|e| format!("ripgrep failed: {}", e))?;

        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8: {}", e))
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
        *self.last_heartbeat.lock_safe() = Instant::now();
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
                    .ok_or_else(|| {
                        TaskError::new(0x0100, "Missing 'query' in payload", &task.id)
                    })?;

                static RT: std::sync::OnceLock<tokio::runtime::Runtime> =
                    std::sync::OnceLock::new();
                let rt = RT.get_or_init(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to build tokio runtime for search")
                });
                match rt.block_on(self.web_search_async(query)) {
                    Ok(results) => Ok(TaskResult::success(
                        results,
                        start.elapsed().as_millis() as u64,
                    )),
                    Err(e) => Ok(TaskResult::failure(e, start.elapsed().as_millis() as u64)),
                }
            }
            "code" => {
                let pattern = task
                    .payload
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        TaskError::new(0x0100, "Missing 'pattern' in payload", &task.id)
                    })?;

                let path = task
                    .payload
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");

                let file_filter = task.payload.get("file_filter").and_then(|v| v.as_str());

                match self.code_search(pattern, path, file_filter) {
                    Ok(results) => Ok(TaskResult::success(
                        results,
                        start.elapsed().as_millis() as u64,
                    )),
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
        *self.last_heartbeat.lock_safe() = Instant::now();
        true
    }
}
