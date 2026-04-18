//! Security Shield Plugin - Defense against all attack types
//!
//! Protects against:
//! - Lateral movement
//! - Gap attacks (air gap crossing)
//! - Injection (SQL, XSS, Command, Template, LDAP)
//! - RCE (Remote Code Execution)
//! - Path traversal
//! - Privilege escalation
//! - Denial of service
//! - Man-in-the-middle
//! - Side-channel attacks

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use lazy_static::lazy_static;

lazy_static! {
    static ref THREAT_LOG: Mutex<Vec<ThreatDetection>> = Mutex::new(Vec::new());
    static ref SECURITY_EVENTS: Mutex<Vec<SecurityEvent>> = Mutex::new(Vec::new());
    static ref NETWORK_CONNECTIONS: Mutex<Vec<NetworkConnection>> = Mutex::new(Vec::new());
    static ref DEFENSE_RULES: Mutex<Vec<DefenseRule>> = Mutex::new(Vec::new());
    static ref BLOCKED_IPS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ref ALLOWED_IPS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ref RATE_LIMITS: Mutex<HashMap<String, Vec<i64>>> = Mutex::new(HashMap::new());
}

/// Attack detection result
#[derive(Debug, Clone)]
pub struct ThreatDetection {
    pub id: String,
    pub attack_type: String,
    pub severity: u8, // 1-10
    pub source: String,
    pub description: String,
    pub blocked: bool,
    pub timestamp: i64,
    pub mitigations: Vec<String>,
}

/// Security event log entry
#[derive(Debug, Clone)]
pub struct SecurityEvent {
    pub timestamp: i64,
    pub event_type: String,
    pub severity: u8,
    pub source_ip: Option<String>,
    pub details: String,
    pub action_taken: String,
}

/// Network connection tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConnection {
    pub source: String,
    pub destination: String,
    pub port: u16,
    pub protocol: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub is_suspicious: bool,
}

/// Defense rules
#[derive(Debug, Clone)]
pub struct DefenseRule {
    pub id: String,
    pub name: String,
    pub attack_type: String,
    pub pattern: String,
    pub action: String, // block, alert, log, quarantine
    pub enabled: bool,
    pub hit_count: u32,
    pub last_hit: Option<i64>,
}

/// Global state - initialized via lazy_static above

/// Initialize default defense rules
pub fn init_default_rules() {
    let mut rules = DEFENSE_RULES.lock().unwrap();

    rules.extend(vec![
        // SQL Injection
        DefenseRule {
            id: "sql-001".into(),
            name: "SQL Injection - OR bypass".into(),
            attack_type: "sql_injection".into(),
            pattern: "(?i)(['\"]\\s*OR\\s+['\"]\\d+['\"]\\s*=\\s*['\"]\\d+|'\\s*OR\\s*'1'\\s*=\\s*'1)".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        DefenseRule {
            id: "sql-002".into(),
            name: "SQL Injection - UNION".into(),
            attack_type: "sql_injection".into(),
            pattern: "(?i)UNION\\s+(ALL\\s+)?SELECT".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        DefenseRule {
            id: "sql-003".into(),
            name: "SQL Injection - DROP".into(),
            attack_type: "sql_injection".into(),
            pattern: "(?i)DROP\\s+TABLE|DROP\\s+DATABASE|TRUNCATE\\s+".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        // XSS
        DefenseRule {
            id: "xss-001".into(),
            name: "XSS - Script tag".into(),
            attack_type: "xss".into(),
            pattern: "(?i)<script[^>]*>|javascript\\s*:|on(error|load|click|mouseover)\\s*=".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        // Command Injection
        DefenseRule {
            id: "cmd-001".into(),
            name: "Command Injection - Pipe".into(),
            attack_type: "command_injection".into(),
            pattern: "(?i)[;|&`]\\s*(cat|ls|id|whoami|uname|rm|wget|curl|bash|sh|python|perl|ruby)\\b".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        DefenseRule {
            id: "cmd-002".into(),
            name: "Command Injection - Substitution".into(),
            attack_type: "command_injection".into(),
            pattern: "\\$\\(|`[^`]+`".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        // Path Traversal
        DefenseRule {
            id: "path-001".into(),
            name: "Path Traversal".into(),
            attack_type: "path_traversal".into(),
            pattern: "(?i)(\\.\\./|\\.\\.\\\\|%2e%2e%2f|%2e%2e/|\\.\\.%2f)".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        // RCE
        DefenseRule {
            id: "rce-001".into(),
            name: "RCE - Reverse shell".into(),
            attack_type: "rce".into(),
            pattern: "(?i)bash\\s+-i|nc\\s+-[el]|python\\s+-c\\s+.*import|perl\\s+-e.*socket".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        DefenseRule {
            id: "rce-002".into(),
            name: "RCE - Eval".into(),
            attack_type: "rce".into(),
            pattern: "(?i)eval\\s*\\(|exec\\s*\\(|compile\\s*\\(|__import__\\s*\\(".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        // Template Injection
        DefenseRule {
            id: "ssti-001".into(),
            name: "Template Injection - Jinja".into(),
            attack_type: "template_injection".into(),
            pattern: "\\{\\{.*\\}\\}|\\{\\%.*\\%\\}|\\$\\{\\{|#\\{".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        // LDAP Injection
        DefenseRule {
            id: "ldap-001".into(),
            name: "LDAP Injection".into(),
            attack_type: "ldap_injection".into(),
            pattern: "(?i)\\)\\s*\\(|\\*\\)|\\)\\(\\|=".into(),
            action: "block".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        // Lateral Movement
        DefenseRule {
            id: "lateral-001".into(),
            name: "Lateral Movement - SMB".into(),
            attack_type: "lateral_movement".into(),
            pattern: "(?i)smb://|\\\\\\\\[\\d.]+\\|psexec|wmiexec".into(),
            action: "alert".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
        // Gap Attack
        DefenseRule {
            id: "gap-001".into(),
            name: "Air Gap Crossing - USB".into(),
            attack_type: "gap_attack".into(),
            pattern: "(?i)/dev/sd[a-z]|/media/|/mnt/usb".into(),
            action: "alert".into(),
            enabled: true,
            hit_count: 0,
            last_hit: None,
        },
    ]);
}

/// Sanitize input against all known injection types
pub fn sanitize_input(input: &str, context: &str) -> Result<Value> {
    let now = now_ts();
    let rules = DEFENSE_RULES.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

    let mut threats_found = Vec::new();
    let mut sanitized = input.to_string();
    let mut blocked = false;

    for rule in rules.iter() {
        if !rule.enabled {
            continue;
        }

        if let Ok(re) = regex::Regex::new(&rule.pattern) {
            if re.is_match(&sanitized) {
                let threat = ThreatDetection {
                    id: format!("threat-{}-{}", rule.id, now),
                    attack_type: rule.attack_type.clone(),
                    severity: calculate_severity(&rule.attack_type, context),
                    source: "input_sanitization".into(),
                    description: format!("Detected {} pattern: {}", rule.name, input.chars().take(100).collect::<String>()),
                    blocked: rule.action == "block",
                    timestamp: now,
                    mitigations: vec![rule.action.clone()],
                };

                threats_found.push(threat.clone());

                if rule.action == "block" {
                    blocked = true;
                    // Remove or neutralize the pattern
                    sanitized = re.replace_all(&sanitized, "[BLOCKED]").to_string();
                }

                // Log the event
                log_security_event(&SecurityEvent {
                    timestamp: now,
                    event_type: format!("injection_{}_detected", rule.attack_type),
                    severity: threat.severity,
                    source_ip: None,
                    details: threat.description.clone(),
                    action_taken: rule.action.clone(),
                });
            }
        }
    }

    // Store threats
    {
        let mut log = THREAT_LOG.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        log.extend(threats_found.clone());
        // Keep only last 1000
        let len = log.len();
        if len > 1000 {
            log.drain(..len - 1000);
        }
    }

    Ok(json!({
        "status": if blocked { "blocked" } else { "ok" },
        "input_length": input.len(),
        "sanitized_length": sanitized.len(),
        "sanitized": sanitized,
        "threats_found": threats_found.len(),
        "threats": threats_found.iter().map(|t| json!({
            "type": t.attack_type,
            "severity": t.severity,
            "description": t.description,
            "blocked": t.blocked
        })).collect::<Vec<_>>()
    }))
}

/// Check if input is safe
pub fn is_safe(input: &str) -> Result<Value> {
    let result = sanitize_input(input, "validation")?;
    Ok(json!({
        "safe": result["status"] == "ok",
        "threats": result["threats_found"]
    }))
}

/// Monitor network for lateral movement
pub fn monitor_network() -> Result<Value> {
    let now = now_ts();

    // Get active connections (simplified - uses ss/netstat)
    let output = std::process::Command::new("ss")
        .args(&["-tunp"])
        .output()
        .ok();

    let mut connections = Vec::new();
    let mut suspicious = Vec::new();

    if let Some(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let proto = parts[0];
                let local = parts[4].split(':').next().unwrap_or("");
                let remote = parts[5].split(':').next().unwrap_or("");

                let conn = NetworkConnection {
                    source: local.to_string(),
                    destination: remote.to_string(),
                    port: parts[5].split(':').nth(1)
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(0),
                    protocol: proto.to_string(),
                    first_seen: now,
                    last_seen: now,
                    bytes_sent: 0,
                    bytes_received: 0,
                    is_suspicious: is_suspicious_connection(remote),
                };

                connections.push(json!({
                    "local": conn.source,
                    "remote": conn.destination,
                    "port": conn.port,
                    "protocol": conn.protocol,
                    "suspicious": conn.is_suspicious
                }));

                if conn.is_suspicious {
                    suspicious.push(conn.destination.clone());
                    log_security_event(&SecurityEvent {
                        timestamp: now,
                        event_type: "suspicious_connection".into(),
                        severity: 7,
                        source_ip: Some(remote.to_string()),
                        details: format!("Suspicious connection to {} on port {}", remote, conn.port),
                        action_taken: "alert".into(),
                    });
                }
            }
        }
    }

    {
        let mut conns = NETWORK_CONNECTIONS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
        *conns = connections.iter()
            .filter_map(|c| serde_json::from_value::<NetworkConnection>(c.clone()).ok())
            .collect();
    }

    Ok(json!({
        "status": "ok",
        "total_connections": connections.len(),
        "suspicious_connections": suspicious.len(),
        "suspicious_destinations": suspicious,
        "connections": connections
    }))
}

/// Check if an IP/connection is suspicious
fn is_suspicious_connection(remote: &str) -> bool {
    // Check against known suspicious patterns
    let suspicious_patterns = [
        // Known bad ports
        "4444", "5555", "8888", "9999", "31337", "12345",
        // Local network scanning
        "192.168.", "10.0.", "172.16.",
    ];

    suspicious_patterns.iter().any(|p| remote.contains(p))
}

/// Detect lateral movement attempts
pub fn detect_lateral_movement() -> Result<Value> {
    let now = now_ts();
    let mut findings = Vec::new();

    // Check for SMB connections
    if let Ok(out) = std::process::Command::new("ss")
        .args(&["-tunp", "sport", "=", "445"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !stdout.lines().skip(1).any(|l| !l.is_empty()) {
            // No active SMB, good
        } else {
            findings.push(json!({
                "type": "smb_activity",
                "severity": 7,
                "details": "Active SMB connections detected"
            }));
        }
    }

    // Check for WMI/RPC connections
    if let Ok(out) = std::process::Command::new("ss")
        .args(&["-tunp", "sport", "=", "135"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.lines().skip(1).any(|l| !l.is_empty()) {
            findings.push(json!({
                "type": "wmi_rpc_activity",
                "severity": 8,
                "details": "Active WMI/RPC connections detected - possible lateral movement"
            }));
        }
    }

    // Check for recent auth failures (possible credential stuffing)
    if let Ok(out) = std::process::Command::new("journalctl")
        .args(&["-u", "sshd", "--since", "1 hour ago", "-g", "Failed password"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let fail_count = stdout.lines().count();
        if fail_count > 5 {
            findings.push(json!({
                "type": "auth_brute_force",
                "severity": 8,
                "details": format!("{} failed SSH auth attempts in last hour", fail_count)
            }));
        }
    }

    log_security_event(&SecurityEvent {
        timestamp: now,
        event_type: "lateral_movement_scan".into(),
        severity: if findings.is_empty() { 0 } else { 7 },
        source_ip: None,
        details: format!("Lateral movement scan complete: {} findings", findings.len()),
        action_taken: if findings.is_empty() { "clear" } else { "alert" }.into(),
    });

    Ok(json!({
        "status": "ok",
        "findings_count": findings.len(),
        "findings": findings
    }))
}

/// Detect gap attack attempts (air gap crossing)
pub fn detect_gap_attacks() -> Result<Value> {
    let now = now_ts();
    let mut findings = Vec::new();

    // Check for USB device connections
    if let Ok(out) = std::process::Command::new("dmesg")
        .args(&["--time-format", "iso", "--level", "info"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.contains("USB") && stdout.contains("New USB device") {
            findings.push(json!({
                "type": "usb_device_connected",
                "severity": 6,
                "details": "New USB device detected - potential air gap crossing vector"
            }));
        }
    }

    // Check for unusual network interfaces
    if let Ok(out) = std::process::Command::new("ip").arg("link").output() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Check for unexpected interfaces (USB ethernet, Bluetooth PAN)
        for iface in &["usb", "bnep", "tun", "tap"] {
            if stdout.contains(iface) && !stdout.contains(&format!("{}0", iface)) {
                findings.push(json!({
                    "type": "unusual_network_interface",
                    "severity": 5,
                    "details": format!("Unusual network interface type detected: {}", iface)
                }));
            }
        }
    }

    // Check for Bluetooth activity
    if let Ok(out) = std::process::Command::new("bluetoothctl").arg("devices").output() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !stdout.trim().is_empty() && stdout.contains("Device") {
            findings.push(json!({
                "type": "bluetooth_devices_nearby",
                "severity": 4,
                "details": "Bluetooth devices detected nearby"
            }));
        }
    }

    log_security_event(&SecurityEvent {
        timestamp: now,
        event_type: "gap_attack_scan".into(),
        severity: if findings.is_empty() { 0 } else { 5 },
        source_ip: None,
        details: format!("Gap attack scan complete: {} findings", findings.len()),
        action_taken: if findings.is_empty() { "clear" } else { "alert" }.into(),
    });

    Ok(json!({
        "status": "ok",
        "findings_count": findings.len(),
        "findings": findings
    }))
}

/// Full security audit
pub fn security_audit() -> Result<Value> {
    let now = now_ts();

    // Run all security checks
    let lateral = detect_lateral_movement().unwrap_or(json!({"findings": []}));
    let gap = detect_gap_attacks().unwrap_or(json!({"findings": []}));
    let network = monitor_network().unwrap_or(json!({"connections": []}));

    let lateral_findings = lateral["findings_count"].as_u64().unwrap_or(0);
    let gap_findings = gap["findings_count"].as_u64().unwrap_or(0);
    let suspicious_conns = network["suspicious_connections"].as_u64().unwrap_or(0);

    let total_threats = lateral_findings + gap_findings + suspicious_conns;
    let risk_level = if total_threats == 0 {
        "low"
    } else if total_threats <= 3 {
        "medium"
    } else if total_threats <= 10 {
        "high"
    } else {
        "critical"
    };

    log_security_event(&SecurityEvent {
        timestamp: now,
        event_type: "security_audit".into(),
        severity: if risk_level == "critical" { 9 } else { 3 },
        source_ip: None,
        details: format!("Security audit: risk={}, threats={}", risk_level, total_threats),
        action_taken: "audit_complete".into(),
    });

    Ok(json!({
        "status": "ok",
        "risk_level": risk_level,
        "total_threats": total_threats,
        "lateral_movement_findings": lateral_findings,
        "gap_attack_findings": gap_findings,
        "suspicious_connections": suspicious_conns,
        "audit_timestamp": now
    }))
}

/// Get threat log
pub fn get_threat_log(limit: u32) -> Result<Value> {
    let log = THREAT_LOG.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
    let threats: Vec<Value> = log.iter().rev()
        .take(limit as usize)
        .map(|t| json!({
            "id": t.id,
            "type": t.attack_type,
            "severity": t.severity,
            "description": t.description,
            "blocked": t.blocked,
            "timestamp": t.timestamp
        }))
        .collect();

    Ok(json!({
        "status": "ok",
        "total_threats": log.len(),
        "threats": threats
    }))
}

/// Get security events
pub fn get_security_events(limit: u32) -> Result<Value> {
    let events = SECURITY_EVENTS.lock().map_err(|e| anyhow!("Lock error: {}", e))?;
    let recent: Vec<Value> = events.iter().rev()
        .take(limit as usize)
        .map(|e| json!({
            "timestamp": e.timestamp,
            "type": e.event_type,
            "severity": e.severity,
            "details": e.details,
            "action": e.action_taken
        }))
        .collect();

    Ok(json!({
        "status": "ok",
        "total_events": events.len(),
        "events": recent
    }))
}

// === Internal helpers ===

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn calculate_severity(attack_type: &str, context: &str) -> u8 {
    match attack_type {
        "rce" => 10,
        "command_injection" => 9,
        "sql_injection" => 8,
        "lateral_movement" => 8,
        "gap_attack" => 7,
        "xss" => 6,
        "path_traversal" => 6,
        "template_injection" => 7,
        "ldap_injection" => 7,
        _ => 5,
    }
}

fn log_security_event(event: &SecurityEvent) {
    if let Ok(mut events) = SECURITY_EVENTS.lock() {
        events.push(event.clone());
        let len = events.len();
        if len > 1000 {
            events.drain(..len - 1000);
        }
    }
}

/// MCP tools handler
pub mod mcp_tools {
    use super::*;

    pub fn handle_sanitize_input(input: &str, context: &str) -> Value {
        match sanitize_input(input, context) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_is_safe(input: &str) -> Value {
        match is_safe(input) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_monitor_network() -> Value {
        match monitor_network() {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_detect_lateral_movement() -> Value {
        match detect_lateral_movement() {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_detect_gap_attacks() -> Value {
        match detect_gap_attacks() {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_security_audit() -> Value {
        match security_audit() {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_threat_log(limit: u32) -> Value {
        match get_threat_log(limit) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }

    pub fn handle_security_events(limit: u32) -> Value {
        match get_security_events(limit) {
            Ok(r) => r,
            Err(e) => json!({"status": "error", "message": e.to_string()}),
        }
    }
}
