//! Remote Control & Self-Management Plugin
//!
//! Capabilities:
//! - Secure messaging between agents
//! - Self-configuration and auto-improvement
//! - Self-repair and error recovery
//! - System monitoring and management
//! - Defense against lateral movement, gap attacks, injection, RCE
//! - Secure file operations
//! - Process and service management

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use lazy_static::lazy_static;

lazy_static! {
    static ref AGENT_REGISTRY: Mutex<HashMap<String, AgentIdentity>> = Mutex::new(HashMap::new());
    static ref MESSAGE_QUEUE: Mutex<Vec<AgentMessage>> = Mutex::new(Vec::new());
    static ref SECURITY_POLICY: Mutex<SecurityPolicy> = Mutex::new(SecurityPolicy {
        allowed_commands: vec!["ls".into(), "cat".into(), "echo".into(), "whoami".into(), "uname".into(), "df".into(), "free".into(), "ps".into()],
        blocked_commands: vec!["rm -rf /".into(), "mkfs".into(), "dd".into(), "fdisk".into(), "chmod 777".into()],
        max_file_size: 100 * 1024 * 1024,
        allowed_paths: vec!["/home".into(), "/tmp".into(), "/var/log".into()],
        blocked_paths: vec!["/etc/shadow".into(), "/etc/passwd".into(), "/root".into()],
        rate_limit_per_minute: 60,
        require_signature: false,
        allow_remote_execution: false,
        max_concurrent_operations: 10,
        audit_all_actions: true,
    });
    static ref SELF_HEAL_RULES: Mutex<Vec<SelfHealRule>> = Mutex::new(Vec::new());
    static ref ACTION_AUDIT: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

/// Secure message between agents
#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub id: String,
    pub from: String,
    pub to: String,
    pub content: String,
    pub message_type: String, // command, response, notification, config_update, learning
    pub timestamp: i64,
    pub signature: Option<String>, // PQC signature
    pub ttl: u32,
    pub priority: u8,
}

/// Agent identity and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub id: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub status: String,
    pub last_heartbeat: i64,
    pub trust_level: u8,
    pub learned_behaviors: HashMap<String, Value>,
}

/// Security policy
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub allowed_commands: Vec<String>,
    pub blocked_commands: Vec<String>,
    pub max_file_size: u64,
    pub allowed_paths: Vec<String>,
    pub blocked_paths: Vec<String>,
    pub rate_limit_per_minute: u32,
    pub require_signature: bool,
    pub allow_remote_execution: bool,
    pub max_concurrent_operations: u32,
    pub audit_all_actions: bool,
}

/// Self-heal rule
#[derive(Debug, Clone)]
pub struct SelfHealRule {
    pub id: String,
    pub trigger_pattern: String,
    pub condition: String,
    pub action: String,
    pub max_retries: u32,
    pub cooldown_seconds: u64,
    pub last_triggered: Option<i64>,
    pub trigger_count: u32,
}

/// Global state - initialized via lazy_static above

/// Register this agent
pub fn agent_register(
    agent_id: &str,
    name: &str,
    capabilities: Vec<String>,
) -> Result<Value> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let agent = AgentIdentity {
        id: agent_id.to_string(),
        name: name.to_string(),
        capabilities,
        status: "active".to_string(),
        last_heartbeat: now,
        trust_level: 50,
        learned_behaviors: HashMap::new(),
    };

    {
        let mut registry = AGENT_REGISTRY.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        registry.insert(agent_id.to_string(), agent);
    }

    audit_action(&format!("agent_register: {}", agent_id));

    Ok(json!({
        "status": "ok",
        "agent_id": agent_id,
        "name": name,
        "message": "Agent registered successfully"
    }))
}

/// Send secure message to another agent
pub fn send_message(
    from: &str,
    to: &str,
    content: &str,
    message_type: &str,
    priority: u8,
) -> Result<Value> {
    // Security check: rate limiting
    if !check_rate_limit(from)? {
        return Ok(json!({
            "status": "error",
            "message": "Rate limit exceeded. Too many messages."
        }));
    }

    // Security check: validate command content
    if !validate_message_content(content)? {
        return Ok(json!({
            "status": "error",
            "message": "Message content blocked by security policy"
        }));
    }

    // Security check: injection detection
    if detect_injection(content) {
        audit_action(&format!("BLOCKED: Injection attempt in message from {}", from));
        return Ok(json!({
            "status": "error",
            "message": "Message blocked: potential injection attack detected"
        }));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let msg_id = format!("msg-{}-{}", from, now);
    let message = AgentMessage {
        id: msg_id.clone(),
        from: from.to_string(),
        to: to.to_string(),
        content: content.to_string(),
        message_type: message_type.to_string(),
        timestamp: now,
        signature: None,
        ttl: 3600,
        priority,
    };

    {
        let mut queue = MESSAGE_QUEUE.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        queue.push(message);
    }

    // If sending to self, process immediately
    if from == to {
        process_self_message(content, message_type)?;
    }

    audit_action(&format!("message_sent: {} -> {} (type: {})", from, to, message_type));

    Ok(json!({
        "status": "ok",
        "message_id": msg_id,
        "from": from,
        "to": to,
        "type": message_type
    }))
}

/// Receive pending messages for an agent
pub fn receive_messages(agent_id: &str, limit: u32) -> Result<Value> {
    let mut queue = MESSAGE_QUEUE.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

    let messages: Vec<Value> = queue
        .iter()
        .filter(|m| m.to == agent_id || m.to == "*")
        .take(limit as usize)
        .map(|m| json!({
            "id": m.id,
            "from": m.from,
            "content": m.content,
            "type": m.message_type,
            "timestamp": m.timestamp,
            "priority": m.priority
        }))
        .collect();

    // Remove delivered messages
    queue.retain(|m| !(m.to == agent_id || m.to == "*"));

    Ok(json!({
        "status": "ok",
        "agent_id": agent_id,
        "message_count": messages.len(),
        "messages": messages
    }))
}

/// Self-configure: update agent settings from learned data
pub fn self_configure(agent_id: &str, config_updates: Value) -> Result<Value> {
    audit_action(&format!("self_configure: {}", agent_id));

    // Validate config doesn't contain dangerous values
    if detect_injection(&serde_json::to_string(&config_updates).unwrap_or_default()) {
        return Ok(json!({
            "status": "error",
            "message": "Configuration blocked: potential injection detected"
        }));
    }

    // Apply config updates to security policy
    if let Some(policy_updates) = config_updates.get("security_policy") {
        let mut policy = SECURITY_POLICY.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        if let Some(allowed) = policy_updates.get("allowed_commands").and_then(|v| v.as_array()) {
            for cmd in allowed {
                if let Some(c) = cmd.as_str() {
                    if !policy.blocked_commands.contains(&c.to_string()) {
                        policy.allowed_commands.push(c.to_string());
                    }
                }
            }
        }

        if let Some(rate) = policy_updates.get("rate_limit").and_then(|v| v.as_u64()) {
            policy.rate_limit_per_minute = rate as u32;
        }
    }

    Ok(json!({
        "status": "ok",
        "agent_id": agent_id,
        "message": "Configuration updated successfully"
    }))
}

/// Self-heal: detect and fix common issues
pub fn self_heal() -> Result<Value> {
    let rules = SELF_HEAL_RULES.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
    let mut actions_taken = Vec::new();

    for rule in rules.iter() {
        // Check if rule should trigger
        let should_trigger = evaluate_heal_condition(&rule.condition)?;

        if should_trigger {
            actions_taken.push(perform_heal_action(&rule.action)?);
        }
    }

    audit_action(&format!("self_heal: {} actions taken", actions_taken.len()));

    Ok(json!({
        "status": "ok",
        "actions_taken": actions_taken.len(),
        "actions": actions_taken
    }))
}

/// Add self-heal rule
pub fn add_heal_rule(
    trigger_pattern: &str,
    condition: &str,
    action: &str,
) -> Result<Value> {
    let rule_id = format!("heal-{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let rule = SelfHealRule {
        id: rule_id.clone(),
        trigger_pattern: trigger_pattern.to_string(),
        condition: condition.to_string(),
        action: action.to_string(),
        max_retries: 3,
        cooldown_seconds: 300,
        last_triggered: None,
        trigger_count: 0,
    };

    {
        let mut rules = SELF_HEAL_RULES.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        rules.push(rule);
    }

    audit_action(&format!("heal_rule_added: {}", rule_id));

    Ok(json!({
        "status": "ok",
        "rule_id": rule_id,
        "message": "Self-heal rule added"
    }))
}

/// Learn from feedback (improve behavior based on results)
pub fn learn_from_feedback(
    agent_id: &str,
    action: &str,
    result: &str,
    success: bool,
) -> Result<Value> {
    let mut registry = AGENT_REGISTRY.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

    if let Some(agent) = registry.get_mut(agent_id) {
        let behavior_key = format!("{}_{}", action, if success { "success" } else { "failure" });
        agent.learned_behaviors.insert(behavior_key, json!({
            "action": action,
            "result": result,
            "success": success,
            "learned_at": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }));

        // Adjust trust level based on success/failure
        if success {
            agent.trust_level = (agent.trust_level + 1).min(100);
        } else {
            agent.trust_level = agent.trust_level.saturating_sub(1);
        }
    }

    audit_action(&format!("learn: {} - {} - success: {}", agent_id, action, success));

    Ok(json!({
        "status": "ok",
        "agent_id": agent_id,
        "action": action,
        "success": success,
        "trust_level": registry.get(agent_id).map(|a| a.trust_level).unwrap_or(0)
    }))
}

/// Execute secure command
pub fn execute_command(command: &str, args: &[&str]) -> Result<Value> {
    // Security check: is command allowed?
    let policy = SECURITY_POLICY.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

    if policy.blocked_commands.iter().any(|b| command.contains(b)) {
        audit_action(&format!("BLOCKED: Blocked command attempt: {}", command));
        return Ok(json!({
            "status": "error",
            "message": "Command blocked by security policy"
        }));
    }

    if !policy.allowed_commands.iter().any(|a| command.contains(a)) {
        audit_action(&format!("BLOCKED: Unapproved command: {}", command));
        return Ok(json!({
            "status": "error",
            "message": "Command not in allowed list"
        }));
    }

    // Check for injection patterns
    for arg in args {
        if detect_injection(arg) {
            audit_action(&format!("BLOCKED: Injection in command args: {}", arg));
            return Ok(json!({
                "status": "error",
                "message": "Command blocked: injection pattern detected in arguments"
            }));
        }
    }

    audit_action(&format!("command: {} {:?}", command, args));

    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|e| anyhow!("Command execution failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok(json!({
        "status": if output.status.success() { "ok" } else { "error" },
        "command": command,
        "exit_code": output.status.code(),
        "stdout": stdout.chars().take(1000).collect::<String>(),
        "stderr": stderr.chars().take(500).collect::<String>()
    }))
}

/// Read file securely (with path validation)
pub fn secure_read_file(path: &str) -> Result<Value> {
    let policy = SECURITY_POLICY.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

    // Validate path
    let path_buf = PathBuf::from(path);

    // Check against blocked paths
    for blocked in &policy.blocked_paths {
        if path.starts_with(blocked) {
            audit_action(&format!("BLOCKED: Access to blocked path: {}", path));
            return Ok(json!({
                "status": "error",
                "message": "Access denied: path is in blocked list"
            }));
        }
    }

    // Check against allowed paths
    let is_allowed = policy.allowed_paths.iter().any(|p| path.starts_with(p));
    if !is_allowed {
        audit_action(&format!("BLOCKED: Access outside allowed paths: {}", path));
        return Ok(json!({
            "status": "error",
            "message": "Access denied: path not in allowed list"
        }));
    }

    // Check file size
    let metadata = fs::metadata(&path_buf).map_err(|e| anyhow!("Cannot stat file: {}", e))?;
    if metadata.len() > policy.max_file_size {
        return Ok(json!({
            "status": "error",
            "message": format!("File too large: {} bytes (max: {})", metadata.len(), policy.max_file_size)
        }));
    }

    audit_action(&format!("read_file: {}", path));

    let content = fs::read_to_string(&path_buf)
        .map_err(|e| anyhow!("Read failed: {}", e))?;

    Ok(json!({
        "status": "ok",
        "path": path,
        "size_bytes": metadata.len(),
        "content": content.chars().take(5000).collect::<String>()
    }))
}

/// Update security policy
pub fn update_security_policy(updates: Value) -> Result<Value> {
    let mut policy = SECURITY_POLICY.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

    if let Some(allowed) = updates.get("allowed_commands").and_then(|v| v.as_array()) {
        policy.allowed_commands = allowed.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }

    if let Some(blocked) = updates.get("blocked_commands").and_then(|v| v.as_array()) {
        policy.blocked_commands = blocked.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }

    if let Some(rate) = updates.get("rate_limit").and_then(|v| v.as_u64()) {
        policy.rate_limit_per_minute = rate as u32;
    }

    audit_action("security_policy_updated");

    Ok(json!({
        "status": "ok",
        "message": "Security policy updated",
        "allowed_commands": policy.allowed_commands,
        "blocked_commands": policy.blocked_commands,
        "rate_limit": policy.rate_limit_per_minute
    }))
}

/// Get security status
pub fn get_security_status() -> Result<Value> {
    let policy = SECURITY_POLICY.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
    let audit = ACTION_AUDIT.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
    let registry = AGENT_REGISTRY.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

    Ok(json!({
        "status": "ok",
        "security_policy": {
            "allowed_commands_count": policy.allowed_commands.len(),
            "blocked_commands_count": policy.blocked_commands.len(),
            "rate_limit_per_minute": policy.rate_limit_per_minute,
            "require_signature": policy.require_signature,
            "allow_remote_execution": policy.allow_remote_execution
        },
        "registered_agents": registry.len(),
        "recent_audit_entries": audit.iter().rev().take(20).collect::<Vec<_>>(),
        "total_audit_entries": audit.len()
    }))
}

// === Internal helpers ===

fn audit_action(action: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let entry = format!("[{}] {}", now, action);

    if let Ok(mut audit) = ACTION_AUDIT.lock() {
        audit.push(entry);
        // Keep only last 1000 entries
        let len = audit.len();
        if len > 1000 {
            audit.drain(..len - 1000);
        }
    }
}

fn check_rate_limit(agent_id: &str) -> Result<bool> {
    let policy = SECURITY_POLICY.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
    let audit = ACTION_AUDIT.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let recent_count = audit.iter()
        .filter(|e| e.contains(agent_id) && e.contains(&now.to_string()[..10].to_string()))
        .count();

    Ok(recent_count < policy.rate_limit_per_minute as usize)
}

fn validate_message_content(content: &str) -> Result<bool> {
    // Block messages with known attack patterns
    let dangerous_patterns = [
        "<script>", "javascript:", "data:text/html",
        "{{7*7}}", "${{", "#{",
        "UNION SELECT", "DROP TABLE", "DELETE FROM",
        "/etc/shadow", "/etc/passwd",
    ];

    let lower = content.to_lowercase();
    for pattern in &dangerous_patterns {
        if lower.contains(&pattern.to_lowercase()) {
            return Ok(false);
        }
    }

    Ok(true)
}

fn detect_injection(input: &str) -> bool {
    // Detect common injection patterns
    let patterns = [
        // SQL injection
        "' OR '1'='1", "\" OR \"1\"=\"1", "UNION SELECT", "DROP TABLE",
        // Command injection
        "; rm -rf", "| cat /etc", "`whoami`", "$(id)",
        // XSS
        "<script>", "javascript:", "onerror=", "onload=",
        // Path traversal
        "../../../", "..\\..\\..", "%2e%2e%2f",
        // Template injection
        "{{7*7}}", "${{", "#{7*7}",
        // RCE patterns
        "bash -i", "nc -e", "python -c import",
    ];

    let lower = input.to_lowercase();
    patterns.iter().any(|p| lower.contains(&p.to_lowercase()))
}

fn process_self_message(content: &str, message_type: &str) -> Result<()> {
    match message_type {
        "command" => {
            // Parse and execute self-command
            let parts: Vec<&str> = content.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let _ = execute_command(parts[0], &[parts[1]]);
            }
        }
        "config_update" => {
            if let Ok(config) = serde_json::from_str::<Value>(content) {
                let _ = self_configure("self", config);
            }
        }
        "learning" => {
            if let Ok(learning) = serde_json::from_str::<Value>(content) {
                if let (Some(action), Some(result), Some(success)) = (
                    learning.get("action").and_then(|v| v.as_str()),
                    learning.get("result").and_then(|v| v.as_str()),
                    learning.get("success").and_then(|v| v.as_bool()),
                ) {
                    let _ = learn_from_feedback("self", action, result, success);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn evaluate_heal_condition(condition: &str) -> Result<bool> {
    // Simple condition evaluation (can be extended)
    match condition {
        "disk_full" => {
            // Check if disk usage > 90%
            let output = Command::new("df").arg("/").output().ok();
            if let Some(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // Parse df output for usage percentage
                if let Some(line) = stdout.lines().nth(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(usage) = parts.get(4) {
                        if let Some(pct) = usage.trim_end_matches('%').parse::<u32>().ok() {
                            return Ok(pct > 90);
                        }
                    }
                }
            }
            Ok(false)
        }
        "memory_high" => {
            let output = Command::new("free").arg("-m").output().ok();
            if let Some(out) = output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(line) = stdout.lines().nth(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        if let (Some(total), Some(used)) = (
                            parts[1].parse::<u64>().ok(),
                            parts[2].parse::<u64>().ok(),
                        ) {
                            return Ok(total > 0 && (used as f64 / total as f64) > 0.9);
                        }
                    }
                }
            }
            Ok(false)
        }
        "service_down" => {
            // Check if key services are running
            let services = ["anydesk", "synapsis-mcp"];
            for svc in services {
                let status = Command::new("systemctl")
                    .arg("is-active")
                    .arg(svc)
                    .output()
                    .ok();
                if let Some(out) = status {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    if stdout.trim() != "active" {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn perform_heal_action(action: &str) -> Result<Value> {
    match action {
        "clear_tmp" => {
            // Clear old temp files
            let _ = Command::new("find").args(&["/tmp", "-mtime", "+1", "-delete"]).output();
            Ok(json!({"action": "clear_tmp", "status": "done"}))
        }
        "restart_service" => {
            // Restart downed services
            let services = ["anydesk", "synapsis-mcp"];
            for svc in services {
                let _ = Command::new("sudo")
                    .args(&["systemctl", "restart", svc])
                    .output();
            }
            Ok(json!({"action": "restart_services", "status": "done"}))
        }
        "free_memory" => {
            let _ = Command::new("sync").output();
            let _ = Command::new("sh").arg("-c").arg("echo 3 > /proc/sys/vm/drop_caches").output();
            Ok(json!({"action": "free_memory", "status": "done"}))
        }
        _ => Ok(json!({"action": action, "status": "unknown"})),
    }
}

/// MCP tools handler
pub mod mcp_tools {
    use super::*;

    pub fn handle_agent_register(agent_id: &str, name: &str, capabilities: Option<&[String]>) -> Value {
        match agent_register(agent_id, name, capabilities.unwrap_or(&[]).to_vec()) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_send_message(from: &str, to: &str, content: &str, msg_type: &str, priority: u8) -> Value {
        match send_message(from, to, content, msg_type, priority) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_receive_messages(agent_id: &str, limit: u32) -> Value {
        match receive_messages(agent_id, limit) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_self_configure(agent_id: &str, config: &Value) -> Value {
        match self_configure(agent_id, config.clone()) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_self_heal() -> Value {
        match self_heal() {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_add_heal_rule(trigger: &str, condition: &str, action: &str) -> Value {
        match add_heal_rule(trigger, condition, action) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_learn(agent_id: &str, action: &str, result: &str, success: bool) -> Value {
        match learn_from_feedback(agent_id, action, result, success) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_execute_command(command: &str, args: &[String]) -> Value {
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        match execute_command(command, &arg_refs) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_secure_read(path: &str) -> Value {
        match secure_read_file(path) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_update_security_policy(updates: &Value) -> Value {
        match update_security_policy(updates.clone()) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_security_status() -> Value {
        match get_security_status() {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }
}
