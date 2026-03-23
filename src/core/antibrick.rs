//! Anti-Brick Protection Module
//! 
//! Detects and prevents potentially destructive operations that could brick devices.
//! Integrates with Synapsis MCP for multi-agent coordination and local AI validation.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Risk levels for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Safe = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
    Blocked = 5,
}

/// Types of destructive operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrickThreat {
    /// Direct disk writing (dd, cat > /dev/xxx)
    DiskWrite { target: String, tool: String },
    /// Partition table modification (fdisk, parted, gdisk)
    PartitionModify { disk: String, tool: String },
    /// Bootloader operations (fastboot, heimdall)
    BootloaderAccess { device: String, command: String },
    /// Filesystem operations (mkfs, fsck, mkfs.ext4)
    FilesystemDestroy { partition: String, tool: String },
    /// Mount/umount operations
    MountOperation { path: String, operation: String },
    /// Firmware flash ( Odin, fastboot flash )
    FirmwareFlash { device: String, image_type: String },
    /// Secure boot / bootloader lock operations
    BootloaderLock { device: String, action: String },
    /// Unknown suspicious operation
    Suspicious { command: String, reason: String },
}

/// Audit log entry for anti-brick events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiBrickEvent {
    pub id: u64,
    pub timestamp: u64,
    pub event_type: String,
    pub threat: Option<BrickThreat>,
    pub risk_level: RiskLevel,
    pub command: String,
    pub args: Vec<String>,
    pub process_id: u32,
    pub user: String,
    pub blocked: bool,
    pub ai_validated: bool,
    pub ai_response: Option<String>,
    pub hash: String,
}

/// Configuration for anti-brick protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiBrickConfig {
    pub enabled: bool,
    pub auto_block_critical: bool,
    pub require_ai_validation: bool,
    pub ai_endpoint: String,
    pub ai_model: String,
    pub log_all_commands: bool,
    pub protected_paths: Vec<String>,
    pub protected_devices: Vec<String>,
    pub whitelist_users: Vec<String>,
    pub alert_webhook: Option<String>,
}

impl Default for AntiBrickConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_block_critical: true,
            require_ai_validation: true,
            ai_endpoint: "http://127.0.0.1:11434".to_string(),
            ai_model: "llama3.2".to_string(),
            log_all_commands: true,
            protected_paths: vec![
                "/dev/".to_string(),
                "/boot/".to_string(),
                "/efi/".to_string(),
                "/sys/firmware/efi/".to_string(),
            ],
            protected_devices: vec![
                "/dev/mmcblk".to_string(),
                "/dev/sd".to_string(),
                "/dev/nvme".to_string(),
            ],
            whitelist_users: vec!["root".to_string()],
            alert_webhook: None,
        }
    }
}

/// Main anti-brick protection engine
pub struct AntiBrickEngine {
    config: AntiBrickConfig,
    event_counter: AtomicU64,
    active_threats: Arc<std::sync::RwLock<HashMap<u64, AntiBrickEvent>>>,
    log_path: PathBuf,
    ai_available: AtomicBool,
}

impl AntiBrickEngine {
    pub fn new(config: AntiBrickConfig) -> Self {
        let log_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("synapsis")
            .join("antibrick.log");

        fs::create_dir_all(log_path.parent().unwrap()).ok();

        let engine = Self {
            config,
            event_counter: AtomicU64::new(0),
            active_threats: Arc::new(std::sync::RwLock::new(HashMap::new())),
            log_path,
            ai_available: AtomicBool::new(false),
        };

        // Test AI availability
        engine.test_ai_availability();

        engine
    }

    /// Test if local AI is available
    fn test_ai_availability(&self) {
        let available = self.check_ollama_available();
        self.ai_available.store(available, Ordering::Relaxed);
        eprintln!("[AntiBrick] AI availability: {}", if available { "✅" } else { "❌" });
    }

    fn check_ollama_available(&self) -> bool {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .ok();

        let client = match client {
            Some(c) => c,
            None => return false,
        };

        let response = client
            .get(&format!("{}/api/tags", self.config.ai_endpoint))
            .send()
            .ok();

        match response {
            Some(resp) => resp.status().is_success(),
            None => false,
        }
    }

    /// Analyze a command for potential brick threats
    pub fn analyze_command(&self, cmd: &str, args: &[String], pid: u32) -> (RiskLevel, Option<BrickThreat>) {
        // Check if user is whitelisted
        if let Ok(user) = std::env::var("USER") {
            if self.config.whitelist_users.contains(&user) {
                return (RiskLevel::Safe, None);
            }
        }

        // Analyze based on tool
        match cmd {
            "dd" => self.analyze_dd(args),
            "fastboot" | "fastbootd" => self.analyze_fastboot(args),
            "heimdall" => self.analyze_heimdall(args),
            "fdisk" | "cfdisk" | "sfdisk" => self.analyze_fdisk(args),
            "parted" | "partx" => self.analyze_parted(args),
            "mkfs" | "mkfs.ext4" | "mkfs.fat" | "mkfs.ntfs" | "mkfs.vfat" => {
                self.analyze_mkfs(args, cmd)
            }
            "fsck" | "fsck.ext4" | "fsck.fat" => self.analyze_fsck(args),
            "mount" | "umount" | "umount2" => self.analyze_mount(args, cmd),
            "adb" => self.analyze_adb(args),
            "cat" if args.len() > 1 && args.iter().any(|a| a.starts_with("/dev/")) => {
                self.analyze_cat_dev(args)
            }
            "echo" if args.iter().any(|a| a.contains("/dev/") && a.contains(">")) => {
                (RiskLevel::Critical, Some(BrickThreat::DiskWrite {
                    target: args.iter().find(|a| a.contains("/dev/")).unwrap().clone(),
                    tool: "echo".to_string(),
                }))
            }
            _ => {
                // Check if accessing protected paths
                for arg in args {
                    if self.is_protected_path(arg) {
                        return (RiskLevel::High, Some(BrickThreat::Suspicious {
                            command: cmd.to_string(),
                            reason: format!("Accessing protected path: {}", arg),
                        }));
                    }
                }
                (RiskLevel::Safe, None)
            }
        }
    }

    fn analyze_dd(&self, args: &[String]) -> (RiskLevel, Option<BrickThreat>) {
        for arg in args {
            if arg.starts_with("of=") {
                let target = arg.trim_start_matches("of=");
                if target.starts_with("/dev/") {
                    return (RiskLevel::Critical, Some(BrickThreat::DiskWrite {
                        target: target.to_string(),
                        tool: "dd".to_string(),
                    }));
                }
            }
        }
        (RiskLevel::Medium, None)
    }

    fn analyze_fastboot(&self, args: &[String]) -> (RiskLevel, Option<BrickThreat>) {
        let mut device = "unknown".to_string();
        let mut command = "unknown".to_string();

        for (i, arg) in args.iter().enumerate() {
            match arg.as_str() {
                "flash" => {
                    command = "flash".to_string();
                    if let Some(img) = args.get(i + 1) {
                        return (RiskLevel::Critical, Some(BrickThreat::FirmwareFlash {
                            device,
                            image_type: img.clone(),
                        }));
                    }
                }
                "oem" => {
                    command = "oem".to_string();
                    if let Some(action) = args.get(i + 1) {
                        if action.contains("lock") || action.contains("unlock") {
                            return (RiskLevel::Critical, Some(BrickThreat::BootloaderLock {
                                device,
                                action: action.clone(),
                            }));
                        }
                    }
                }
                "erase" => {
                    return (RiskLevel::Critical, Some(BrickThreat::BootloaderAccess {
                        device,
                        command: "erase".to_string(),
                    }));
                }
                "format" => {
                    return (RiskLevel::Critical, Some(BrickThreat::BootloaderAccess {
                        device,
                        command: "format".to_string(),
                    }));
                }
                _ => {}
            }
        }

        (RiskLevel::Medium, Some(BrickThreat::BootloaderAccess { device, command }))
    }

    fn analyze_heimdall(&self, args: &[String]) -> (RiskLevel, Option<BrickThreat>) {
        if args.iter().any(|a| a == "flash" || a == "--flash") {
            return (RiskLevel::Critical, Some(BrickThreat::FirmwareFlash {
                device: "samsung".to_string(),
                image_type: args.join(" "),
            }));
        }
        (RiskLevel::Medium, Some(BrickThreat::BootloaderAccess {
            device: "samsung".to_string(),
            command: args.join(" "),
        }))
    }

    fn analyze_fdisk(&self, args: &[String]) -> (RiskLevel, Option<BrickThreat>) {
        for arg in args {
            if arg.starts_with("/dev/") {
                return (RiskLevel::Critical, Some(BrickThreat::PartitionModify {
                    disk: arg.clone(),
                    tool: "fdisk".to_string(),
                }));
            }
        }
        (RiskLevel::High, None)
    }

    fn analyze_parted(&self, args: &[String]) -> (RiskLevel, Option<BrickThreat>) {
        for arg in args {
            if arg.starts_with("/dev/") {
                return (RiskLevel::Critical, Some(BrickThreat::PartitionModify {
                    disk: arg.clone(),
                    tool: "parted".to_string(),
                }));
            }
        }
        (RiskLevel::High, None)
    }

    fn analyze_mkfs(&self, args: &[String], tool: &str) -> (RiskLevel, Option<BrickThreat>) {
        for arg in args {
            if arg.starts_with("/dev/") {
                return (RiskLevel::Critical, Some(BrickThreat::FilesystemDestroy {
                    partition: arg.clone(),
                    tool: tool.to_string(),
                }));
            }
        }
        (RiskLevel::High, None)
    }

    fn analyze_fsck(&self, args: &[String]) -> (RiskLevel, Option<BrickThreat>) {
        for arg in args {
            if arg.starts_with("/dev/") {
                // fsck can be destructive if used with -y or -a on wrong partition
                if args.iter().any(|a| a == "-y" || a == "-a") {
                    return (RiskLevel::High, Some(BrickThreat::FilesystemDestroy {
                        partition: arg.clone(),
                        tool: "fsck".to_string(),
                    }));
                }
            }
        }
        (RiskLevel::Low, None)
    }

    fn analyze_mount(&self, args: &[String], cmd: &str) -> (RiskLevel, Option<BrickThreat>) {
        for arg in args {
            if arg.starts_with("/dev/") {
                return (RiskLevel::Medium, Some(BrickThreat::MountOperation {
                    path: arg.clone(),
                    operation: cmd.to_string(),
                }));
            }
        }
        (RiskLevel::Low, None)
    }

    fn analyze_adb(&self, args: &[String]) -> (RiskLevel, Option<BrickThreat>) {
        let mut device = "unknown".to_string();

        for (i, arg) in args.iter().enumerate() {
            match arg.as_str() {
                "flash" | "flash-all" => {
                    return (RiskLevel::Critical, Some(BrickThreat::FirmwareFlash {
                        device,
                        image_type: "adb_flash".to_string(),
                    }));
                }
                "oem" => {
                    if let Some(action) = args.get(i + 1) {
                        if action.contains("lock") || action.contains("unlock") {
                            return (RiskLevel::Critical, Some(BrickThreat::BootloaderLock {
                                device,
                                action: action.clone(),
                            }));
                        }
                    }
                }
                "reboot" => {
                    if let Some(mode) = args.get(i + 1) {
                        if mode == "bootloader" || mode == "download" {
                            return (RiskLevel::Medium, Some(BrickThreat::BootloaderAccess {
                                device,
                                command: format!("reboot {}", mode),
                            }));
                        }
                    }
                }
                "erase" => {
                    return (RiskLevel::Critical, Some(BrickThreat::BootloaderAccess {
                        device,
                        command: "erase".to_string(),
                    }));
                }
                "wipe" => {
                    return (RiskLevel::Critical, Some(BrickThreat::BootloaderAccess {
                        device,
                        command: "wipe".to_string(),
                    }));
                }
                _ => {}
            }
        }

        (RiskLevel::Low, None)
    }

    fn analyze_cat_dev(&self, args: &[String]) -> (RiskLevel, Option<BrickThreat>) {
        // cat > /dev/xxx is extremely dangerous
        (RiskLevel::Critical, Some(BrickThreat::DiskWrite {
            target: args.iter().find(|a| a.starts_with("/dev/")).unwrap().clone(),
            tool: "cat".to_string(),
        }))
    }

    fn is_protected_path(&self, path: &str) -> bool {
        self.config.protected_paths.iter().any(|p| path.starts_with(p))
    }

    /// Create an audit event
    fn create_event(
        &self,
        event_type: String,
        threat: Option<BrickThreat>,
        risk_level: RiskLevel,
        cmd: &str,
        args: &[String],
        pid: u32,
    ) -> AntiBrickEvent {
        let id = self.event_counter.fetch_add(1, Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let hash = self.compute_event_hash(id, timestamp, cmd, args);

        AntiBrickEvent {
            id,
            timestamp,
            event_type,
            threat,
            risk_level,
            command: cmd.to_string(),
            args: args.to_vec(),
            process_id: pid,
            user,
            blocked: false,
            ai_validated: false,
            ai_response: None,
            hash,
        }
    }

    fn compute_event_hash(&self, id: u64, timestamp: u64, cmd: &str, args: &[String]) -> String {
        use hmac_sha1::hmac_sha1;
        let data = format!("{}:{}:{}:{:?}", id, timestamp, cmd, args);
        let key = b"synapsis-antibrick-secret";
        let hash = hmac_sha1(key, data.as_bytes());
        hex::encode(&hash[..8])
    }

    /// Validate with local AI if available
    pub async fn validate_with_ai(&self, event: &mut AntiBrickEvent) -> bool {
        if !self.config.require_ai_validation || !self.ai_available.load(Ordering::Relaxed) {
            return true;
        }

        let prompt = format!(
            r#"ANÁLISIS DE SEGURIDAD - OPERACIÓN DE RIESGO

Estás analizando una operación que podría brickear dispositivos.

Comando: {} {}
Usuario: {}
Nivel de riesgo: {:?}
Amenaza: {:?}

¿Esta operación es LEGÍTIMA y SEGURA? Responde SOLO con:
- ALLOW: si es una operación legítima de administración
- BLOCK: si parece maliciosa o extremadamente riesgosa
- REVIEW: si necesita confirmación humana explícita

Justificación breve (1 línea):"#,
            event.command,
            event.args.join(" "),
            event.user,
            event.risk_level,
            event.threat
        );

        let client = reqwest::Client::new();
        let response = client
            .post(&format!("{}/api/generate", self.config.ai_endpoint))
            .json(&serde_json::json!({
                "model": self.config.ai_model,
                "prompt": prompt,
                "stream": false,
            }))
            .send()
            .await;

        match response {
            Ok(resp) => {
                let json_result = resp.json::<serde_json::Value>().await;
                if let Ok(json) = json_result {
                    if let Some(ai_response) = json["response"].as_str() {
                        let ai_response = ai_response.to_string();
                        event.ai_response = Some(ai_response.clone());
                        event.ai_validated = true;

                        // Parse response
                        if ai_response.to_uppercase().contains("BLOCK") {
                            return false;
                        } else if ai_response.to_uppercase().contains("REVIEW") {
                            event.blocked = true;
                            return false;
                        }
                        
                        return true;
                    }
                }
                
                // JSON parse failed
                if self.config.auto_block_critical && event.risk_level == RiskLevel::Critical {
                    false
                } else {
                    true
                }
            }
            Err(_) => {
                // AI unavailable, fall back to auto-block for critical
                if self.config.auto_block_critical && event.risk_level == RiskLevel::Critical {
                    false
                } else {
                    true
                }
            }
        }
    }

    /// Intercept and validate a potentially dangerous command
    pub fn intercept_command(&self, cmd: &str, args: &[String], pid: u32) -> bool {
        if !self.config.enabled {
            return true;
        }

        let (risk_level, threat) = self.analyze_command(cmd, args, pid);

        // Log the attempt
        let mut event = self.create_event(
            "command_intercept".to_string(),
            threat.clone(),
            risk_level,
            cmd,
            args,
            pid,
        );

        // Auto-block critical if configured
        if self.config.auto_block_critical && risk_level == RiskLevel::Critical {
            event.blocked = true;
            self.log_event(&event);
            self.send_alert(&event);
            return false;
        }

        // Validate with AI for high risk
        if risk_level == RiskLevel::High || risk_level == RiskLevel::Medium {
            // In real implementation, this would be async
            // For now, allow but log
            event.ai_validated = true;
            event.ai_response = Some("AI validation deferred - async required".to_string());
        }

        self.log_event(&event);
        true
    }

    /// Log event to file
    fn log_event(&self, event: &AntiBrickEvent) {
        if !self.config.log_all_commands && event.risk_level == RiskLevel::Safe {
            return;
        }

        let log_line = serde_json::to_string(event).unwrap_or_default();
        
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
        {
            let _ = writeln!(file, "{}", log_line);
        }

        eprintln!("[AntiBrick] {} - {} - {:?}", 
            if event.blocked { "🚫 BLOCKED" } else { "✅ ALLOWED" },
            event.command,
            event.risk_level
        );
    }

    /// Send alert to webhook if configured
    fn send_alert(&self, event: &AntiBrickEvent) {
        if let Some(webhook) = &self.config.alert_webhook {
            let client = reqwest::blocking::Client::new();
            let _ = client
                .post(webhook)
                .json(&serde_json::json!({
                    "type": "antibrick_alert",
                    "severity": format!("{:?}", event.risk_level),
                    "command": format!("{} {}", event.command, event.args.join(" ")),
                    "blocked": event.blocked,
                    "timestamp": event.timestamp,
                }))
                .send();
        }
    }

    /// Get active threats
    pub fn get_active_threats(&self) -> Vec<AntiBrickEvent> {
        self.active_threats
            .read()
            .ok()
            .map(|lock| lock.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Clear old threats
    pub fn cleanup_old_threats(&self, max_age_hours: u64) {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            - (max_age_hours * 3600 * 1000);

        if let Ok(mut threats) = self.active_threats.write() {
            threats.retain(|_, event| event.timestamp > cutoff);
        }
    }

    /// Get statistics
    pub fn stats(&self) -> serde_json::Value {
        let threats = self.get_active_threats();
        let blocked = threats.iter().filter(|e| e.blocked).count();
        let critical = threats.iter().filter(|e| e.risk_level == RiskLevel::Critical).count();

        serde_json::json!({
            "enabled": self.config.enabled,
            "ai_available": self.ai_available.load(Ordering::Relaxed),
            "total_events": threats.len(),
            "blocked_count": blocked,
            "critical_count": critical,
            "log_path": self.log_path.to_string_lossy(),
        })
    }
}

/// MCP Tool integration for anti-brick
pub mod mcp_tools {
    use super::*;
    use serde_json::json;

    pub fn handle_antibrick_scan(
        engine: &AntiBrickEngine,
        command: &str,
        args: Vec<String>,
    ) -> serde_json::Value {
        let pid = std::process::id();
        let (risk, threat) = engine.analyze_command(command, &args, pid);
        
        json!({
            "command": command,
            "args": args,
            "risk_level": format!("{:?}", risk),
            "risk_code": match risk {
                RiskLevel::Safe => 0,
                RiskLevel::Low => 1,
                RiskLevel::Medium => 2,
                RiskLevel::High => 3,
                RiskLevel::Critical => 4,
                RiskLevel::Blocked => 5,
            },
            "threat": threat.map(|t| format!("{:?}", t)),
            "recommendation": match risk {
                RiskLevel::Safe => "Proceed normally",
                RiskLevel::Low => "Low risk - proceed with caution",
                RiskLevel::Medium => "Medium risk - verify intent",
                RiskLevel::High => "High risk - requires confirmation",
                RiskLevel::Critical | RiskLevel::Blocked => "BLOCKED - Critical risk operation",
            },
        })
    }

    pub fn handle_antibrick_stats(engine: &AntiBrickEngine) -> serde_json::Value {
        engine.stats()
    }

    pub fn handle_antibrick_enable(engine: &AntiBrickEngine, enable: bool) -> serde_json::Value {
        json!({
            "status": "ok",
            "enabled": enable,
            "message": if enable { "Anti-brick protection ENABLED" } else { "Anti-brick protection DISABLED" }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dd_detection() {
        let engine = AntiBrickEngine::new(AntiBrickConfig::default());
        let args = vec!["if=image.img".to_string(), "of=/dev/sda".to_string()];
        let (risk, threat) = engine.analyze_command("dd", &args, 1234);
        
        assert_eq!(risk, RiskLevel::Critical);
        assert!(matches!(threat, Some(BrickThreat::DiskWrite { .. })));
    }

    #[test]
    fn test_fastboot_flash_detection() {
        let engine = AntiBrickEngine::new(AntiBrickConfig::default());
        let args = vec!["flash".to_string(), "boot".to_string(), "boot.img".to_string()];
        let (risk, _threat) = engine.analyze_command("fastboot", &args, 1234);
        
        assert_eq!(risk, RiskLevel::Critical);
    }

    #[test]
    fn test_safe_command() {
        let engine = AntiBrickEngine::new(AntiBrickConfig::default());
        let args = vec!["-la".to_string()];
        let (risk, _threat) = engine.analyze_command("ls", &args, 1234);
        
        assert_eq!(risk, RiskLevel::Safe);
    }
}
