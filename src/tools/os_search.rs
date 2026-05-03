//! Multi-OS Search Tool for Synapsis
//! Supports local (Linux/Windows/macOS), Android (ADB), and Remote (SSH) searches.

use anyhow::Result;
use serde_json::{json, Value};
use std::process::Command;

/// Perform a file system search across different OS targets
pub fn os_search(
    target: &str,
    path: &str,
    pattern: &str,
    ssh_target: Option<&str>,
) -> Result<Value> {
    if path.is_empty() || pattern.is_empty() {
        return Ok(json!({
            "status": "error",
            "message": "Path and pattern cannot be empty"
        }));
    }

    let output = match target.to_lowercase().as_str() {
        "local" => {
            // Use 'find' on Unix-like systems, or 'where /r' on Windows (simulated here with find)
            Command::new("find")
                .arg(path)
                .arg("-name")
                .arg(pattern)
                .output()?
        }
        "adb" => {
            // Search on Android via ADB
            Command::new("adb")
                .arg("shell")
                .arg(format!("find {} -name '{}'", path, pattern))
                .output()?
        }
        "ssh" => {
            if let Some(host) = ssh_target {
                // Search on remote system via SSH
                Command::new("ssh")
                    .arg(host)
                    .arg(format!("find {} -name '{}'", path, pattern))
                    .output()?
            } else {
                return Ok(json!({
                    "status": "error",
                    "message": "SSH target host is required for ssh searches"
                }));
            }
        }
        _ => {
            return Ok(json!({
                "status": "error",
                "message": format!("Unsupported target: {}", target)
            }));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(json!({
            "status": "error",
            "message": format!("Search failed: {}", stderr)
        }));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    Ok(json!({
        "status": "ok",
        "target": target,
        "path": path,
        "pattern": pattern,
        "total_matches": results.len(),
        "matches": results
    }))
}

/// MCP tools handler
pub mod mcp_tools {
    use super::*;

    pub fn handle_os_search(
        target: &str,
        path: &str,
        pattern: &str,
        ssh_target: Option<&str>,
    ) -> Value {
        match os_search(target, path, pattern, ssh_target) {
            Ok(result) => result,
            Err(e) => json!({
                "status": "error",
                "message": format!("OS Search failed: {}", e)
            }),
        }
    }
}
