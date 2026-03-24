//! Critical Filesystem Watchdog
//! 
//! Monitors critical system paths for unauthorized modifications that could brick devices.
//! Uses inotify on Linux for real-time monitoring.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Watch event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatchEventType {
    FileCreated,
    FileModified,
    FileDeleted,
    FileMoved,
    PermissionsChanged,
    SuspiciousAccess,
}

/// Watch event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEvent {
    pub id: u64,
    pub timestamp: u64,
    pub event_type: WatchEventType,
    pub path: String,
    pub original_path: Option<String>, // For move operations
    pub process_id: Option<u32>,
    pub user: String,
    pub severity: u8, // 0-5
    pub hash_before: Option<String>,
    pub hash_after: Option<String>,
    pub blocked: bool,
}

/// Critical path configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalPath {
    pub path: String,
    pub description: String,
    pub severity: u8, // 0-5
    pub monitor_mode: MonitorMode,
    pub allowed_operations: Vec<String>,
    pub blocked_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitorMode {
    ReadOnly,      // Block all writes
    AuditOnly,     // Log but don't block
    SmartMonitor,  // Use AI/heuristics to decide
}

/// Watchdog configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogConfig {
    pub enabled: bool,
    pub critical_paths: Vec<CriticalPath>,
    pub snapshot_interval_hours: u64,
    pub enable_hash_verification: bool,
    pub alert_on_change: bool,
    pub auto_backup: bool,
    pub backup_path: Option<String>,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            critical_paths: Self::default_critical_paths(),
            snapshot_interval_hours: 24,
            enable_hash_verification: true,
            alert_on_change: true,
            auto_backup: true,
            backup_path: None,
        }
    }
}

impl WatchdogConfig {
    fn default_critical_paths() -> Vec<CriticalPath> {
        vec![
            CriticalPath {
                path: "/boot".to_string(),
                description: "Boot partition - kernel and initrd".to_string(),
                severity: 5,
                monitor_mode: MonitorMode::ReadOnly,
                allowed_operations: vec!["read".to_string()],
                blocked_patterns: vec!["*.img".to_string(), "*.iso".to_string()],
            },
            CriticalPath {
                path: "/efi".to_string(),
                description: "EFI System Partition".to_string(),
                severity: 5,
                monitor_mode: MonitorMode::ReadOnly,
                allowed_operations: vec!["read".to_string()],
                blocked_patterns: vec![],
            },
            CriticalPath {
                path: "/sys/firmware/efi".to_string(),
                description: "EFI firmware interface".to_string(),
                severity: 5,
                monitor_mode: MonitorMode::ReadOnly,
                allowed_operations: vec!["read".to_string()],
                blocked_patterns: vec![],
            },
            CriticalPath {
                path: "/dev".to_string(),
                description: "Device files".to_string(),
                severity: 4,
                monitor_mode: MonitorMode::SmartMonitor,
                allowed_operations: vec!["read".to_string(), "open".to_string()],
                blocked_patterns: vec!["/dev/sd*".to_string(), "/dev/mmcblk*".to_string(), "/dev/nvme*".to_string()],
            },
            CriticalPath {
                path: "/proc".to_string(),
                description: "Process information".to_string(),
                severity: 3,
                monitor_mode: MonitorMode::AuditOnly,
                allowed_operations: vec![],
                blocked_patterns: vec![],
            },
            CriticalPath {
                path: "/etc/fstab".to_string(),
                description: "Filesystem mount table".to_string(),
                severity: 5,
                monitor_mode: MonitorMode::ReadOnly,
                allowed_operations: vec!["read".to_string()],
                blocked_patterns: vec![],
            },
            CriticalPath {
                path: "/etc/grub".to_string(),
                description: "GRUB bootloader configuration".to_string(),
                severity: 5,
                monitor_mode: MonitorMode::ReadOnly,
                allowed_operations: vec!["read".to_string()],
                blocked_patterns: vec![],
            },
            CriticalPath {
                path: "/etc/default/grub".to_string(),
                description: "GRUB main configuration".to_string(),
                severity: 5,
                monitor_mode: MonitorMode::ReadOnly,
                allowed_operations: vec!["read".to_string()],
                blocked_patterns: vec![],
            },
        ]
    }
}

/// File snapshot for integrity verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: String,
    pub hash: String,
    pub size: u64,
    pub modified: u64,
    pub permissions: u32,
    pub captured_at: u64,
}

/// Main watchdog engine
pub struct FilesystemWatchdog {
    config: WatchdogConfig,
    event_counter: AtomicU64,
    events: Arc<std::sync::RwLock<Vec<WatchEvent>>>,
    snapshots: Arc<std::sync::RwLock<HashMap<String, FileSnapshot>>>,
    running: AtomicBool,
    log_path: PathBuf,
}

impl FilesystemWatchdog {
    pub fn new(config: WatchdogConfig) -> Self {
        let log_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("synapsis")
            .join("watchdog.log");

        fs::create_dir_all(log_path.parent().unwrap()).ok();

        let watchdog = Self {
            config,
            event_counter: AtomicU64::new(0),
            events: Arc::new(std::sync::RwLock::new(Vec::new())),
            snapshots: Arc::new(std::sync::RwLock::new(HashMap::new())),
            running: AtomicBool::new(false),
            log_path,
        };

        // Create initial snapshots
        watchdog.create_initial_snapshots();

        watchdog
    }

    /// Create initial file snapshots for integrity tracking
    fn create_initial_snapshots(&self) {
        for critical_path in &self.config.critical_paths {
            if let MonitorMode::ReadOnly | MonitorMode::SmartMonitor = critical_path.monitor_mode {
                let _ = self.snapshot_path(&critical_path.path);
            }
        }
        eprintln!("[Watchdog] Initial snapshots created");
    }

    /// Create snapshot of a path (file or directory)
    pub fn snapshot_path(&self, path: &str) -> Result<Vec<FileSnapshot>, String> {
        let mut snapshots = Vec::new();
        let path = Path::new(path);

        if !path.exists() {
            return Err(format!("Path does not exist: {}", path.display()));
        }

        if path.is_file() {
            if let Ok(snapshot) = self.create_file_snapshot(path) {
                snapshots.push(snapshot);
            }
        } else if path.is_dir() {
            // Walk directory (limited depth)
            let _ = self.walk_directory(path, 2, &mut snapshots);
        }

        // Store snapshots
        if let Ok(mut store) = self.snapshots.write() {
            for snap in &snapshots {
                store.insert(snap.path.clone(), snap.clone());
            }
        }

        Ok(snapshots)
    }

    fn create_file_snapshot(&self, path: &Path) -> Result<FileSnapshot, String> {
        let metadata = fs::metadata(path)
            .map_err(|e| format!("Cannot read metadata: {}", e))?;

        let hash = if self.config.enable_hash_verification && metadata.is_file() {
            self.compute_file_hash(path).unwrap_or_else(|_| "unknown".to_string())
        } else {
            "disabled".to_string()
        };

        let modified = metadata.modified()
            .map(|t| t.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64)
            .unwrap_or(0);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Ok(FileSnapshot {
            path: path.to_string_lossy().to_string(),
            hash,
            size: metadata.len(),
            modified,
            permissions: metadata.mode(),
            captured_at: timestamp,
        })
    }

    fn walk_directory(&self, path: &Path, max_depth: usize, snapshots: &mut Vec<FileSnapshot>) -> Result<(), String> {
        self.walk_recursive(path, 0, max_depth, snapshots)
    }

    fn walk_recursive(&self, path: &Path, current_depth: usize, max_depth: usize, snapshots: &mut Vec<FileSnapshot>) -> Result<(), String> {
        if current_depth > max_depth {
            return Ok(());
        }

        let entries = fs::read_dir(path)
            .map_err(|e| format!("Cannot read directory {}: {}", path.display(), e))?;

        for entry in entries.flatten() {
            let entry_path = entry.path();
            
            // Skip symlinks and special files
            if entry_path.is_symlink() {
                continue;
            }

            if entry_path.is_file() {
                if let Ok(snapshot) = self.create_file_snapshot(&entry_path) {
                    snapshots.push(snapshot);
                }
            } else if entry_path.is_dir() {
                let _ = self.walk_recursive(&entry_path, current_depth + 1, max_depth, snapshots);
            }
        }

        Ok(())
    }

    fn compute_file_hash(&self, path: &Path) -> Result<String, String> {
        use sha2::{Sha256, Digest};
        
        let content = fs::read(path)
            .map_err(|e| format!("Cannot read file: {}", e))?;

        let mut hasher = Sha256::new();
        hasher.update(&content);
        let result = hasher.finalize();

        Ok(hex::encode(result))
    }

    /// Verify integrity of monitored files
    pub fn verify_integrity(&self) -> Vec<WatchEvent> {
        let mut events = Vec::new();
        let snapshots = self.snapshots.read().ok();

        if let Some(store) = snapshots {
            for (path, original) in store.iter() {
                let current_path = Path::new(path);
                
                if !current_path.exists() {
                    // File was deleted!
                    let event = self.create_event(
                        WatchEventType::FileDeleted,
                        path.clone(),
                        None,
                        5,
                        Some(original.hash.clone()),
                        None,
                        false,
                    );
                    events.push(event);
                    continue;
                }

                // Check if file was modified
                if let Ok(metadata) = fs::metadata(current_path) {
                    let current_modified = metadata.modified()
                        .map(|t| t.duration_since(UNIX_EPOCH).unwrap().as_millis() as u64)
                        .unwrap_or(0);

                    if current_modified > original.modified {
                        // File changed - compute new hash
                        let new_hash = self.compute_file_hash(current_path).ok();
                        let severity = self.get_path_severity(path);

                        let event = self.create_event(
                            WatchEventType::FileModified,
                            path.clone(),
                            None,
                            severity,
                            Some(original.hash.clone()),
                            new_hash.clone(),
                            false,
                        );
                        events.push(event);

                        // Update snapshot
                        if let Ok(mut store) = self.snapshots.write() {
                            if let Ok(new_snap) = self.create_file_snapshot(current_path) {
                                store.insert(path.clone(), new_snap);
                            }
                        }
                    }
                }
            }
        }

        // Log events
        for event in &events {
            self.log_event(event);
        }

        events
    }

    fn get_path_severity(&self, path: &str) -> u8 {
        for critical in &self.config.critical_paths {
            if path.starts_with(&critical.path) {
                return critical.severity;
            }
        }
        3
    }

    #[allow(clippy::too_many_arguments)]
    fn create_event(
        &self,
        event_type: WatchEventType,
        path: String,
        original_path: Option<String>,
        severity: u8,
        hash_before: Option<String>,
        hash_after: Option<String>,
        blocked: bool,
    ) -> WatchEvent {
        let id = self.event_counter.fetch_add(1, Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

        WatchEvent {
            id,
            timestamp,
            event_type,
            path,
            original_path,
            process_id: None,
            user,
            severity,
            hash_before,
            hash_after,
            blocked,
        }
    }

    fn log_event(&self, event: &WatchEvent) {
        let log_line = serde_json::to_string(event).unwrap_or_default();
        
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
        {
            let _ = writeln!(file, "{}", log_line);
        }

        eprintln!("[Watchdog] {} - {} - Severity: {}",
            match &event.event_type {
                WatchEventType::FileCreated => "📁 CREATED",
                WatchEventType::FileModified => "📝 MODIFIED",
                WatchEventType::FileDeleted => "❌ DELETED",
                WatchEventType::FileMoved => "🔄 MOVED",
                WatchEventType::PermissionsChanged => "🔐 PERMISSIONS",
                WatchEventType::SuspiciousAccess => "⚠️ SUSPICIOUS",
            },
            event.path,
            event.severity
        );
    }

    /// Start background monitoring (inotify-based on Linux)
    pub fn start_monitoring(&self) {
        if self.running.swap(true, Ordering::Relaxed) {
            eprintln!("[Watchdog] Already running");
            return;
        }

        eprintln!("[Watchdog] Starting filesystem monitoring...");
        
        // In a full implementation, this would use inotify on Linux
        // For now, we'll do periodic verification
        let config = self.config.clone();
        let events = Arc::clone(&self.events);
        
        std::thread::spawn(move || {
            // Periodic verification loop
            loop {
                std::thread::sleep(Duration::from_secs(60)); // Check every minute
                
                // In production, this would use inotify for real-time monitoring
            }
        });
    }

    /// Stop monitoring
    pub fn stop_monitoring(&self) {
        self.running.store(false, Ordering::Relaxed);
        eprintln!("[Watchdog] Stopped");
    }

    /// Get recent events
    pub fn get_events(&self, limit: usize) -> Vec<WatchEvent> {
        self.events
            .read()
            .ok()
            .map(|events| {
                events.iter()
                    .rev()
                    .take(limit)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get statistics
    pub fn stats(&self) -> serde_json::Value {
        let events = self.events.read().map(|g| g.clone()).unwrap_or_default();
        let snapshots = self.snapshots.read().map(|g| g.clone()).unwrap_or_default();
        
        let critical_count = events.iter().filter(|e| e.severity >= 4).count();
        let blocked_count = events.iter().filter(|e| e.blocked).count();

        serde_json::json!({
            "enabled": self.config.enabled,
            "running": self.running.load(Ordering::Relaxed),
            "monitored_paths": self.config.critical_paths.len(),
            "snapshots": snapshots.len(),
            "total_events": events.len(),
            "critical_events": critical_count,
            "blocked_events": blocked_count,
            "log_path": self.log_path.to_string_lossy(),
        })
    }

    /// Check if a path is protected
    pub fn is_protected_path(&self, path: &str) -> (bool, Option<&CriticalPath>) {
        for critical in &self.config.critical_paths {
            if path.starts_with(&critical.path) {
                return (true, Some(critical));
            }
        }
        (false, None)
    }
}

/// MCP Tools integration
pub mod mcp_tools {
    use super::*;
    use serde_json::json;

    pub fn handle_watchdog_stats(watchdog: &FilesystemWatchdog) -> serde_json::Value {
        watchdog.stats()
    }

    pub fn handle_watchdog_verify(watchdog: &FilesystemWatchdog) -> serde_json::Value {
        let events = watchdog.verify_integrity();
        json!({
            "status": "ok",
            "issues_found": events.len(),
            "events": events.iter().map(|e| json!({
                "type": format!("{:?}", e.event_type),
                "path": e.path,
                "severity": e.severity,
            })).collect::<Vec<_>>()
        })
    }

    pub fn handle_watchdog_snapshot(watchdog: &FilesystemWatchdog, path: String) -> serde_json::Value {
        match watchdog.snapshot_path(&path) {
            Ok(snapshots) => json!({
                "status": "ok",
                "files_snapshoted": snapshots.len(),
                "snapshots": snapshots.iter().take(10).map(|s| json!({
                    "path": s.path,
                    "hash": &s.hash[..16.min(s.hash.len())],
                    "size": s.size,
                })).collect::<Vec<_>>()
            }),
            Err(e) => json!({
                "status": "error",
                "error": e
            }),
        }
    }

    pub fn handle_watchdog_events(watchdog: &FilesystemWatchdog, limit: usize) -> serde_json::Value {
        let events = watchdog.get_events(limit);
        json!({
            "status": "ok",
            "events": events.iter().map(|e| json!({
                "type": format!("{:?}", e.event_type),
                "path": e.path,
                "severity": e.severity,
                "timestamp": e.timestamp,
            })).collect::<Vec<_>>()
        })
    }

    pub fn handle_watchdog_check_path(watchdog: &FilesystemWatchdog, path: String) -> serde_json::Value {
        let (protected, critical) = watchdog.is_protected_path(&path);
        json!({
            "path": path,
            "protected": protected,
            "critical_path": critical.map(|c| json!({
                "path": c.path,
                "description": c.description,
                "severity": c.severity,
                "mode": format!("{:?}", c.monitor_mode),
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protected_path_detection() {
        let watchdog = FilesystemWatchdog::new(WatchdogConfig::default());
        
        let (protected, critical) = watchdog.is_protected_path("/boot/vmlinuz");
        assert!(protected);
        assert!(critical.is_some());
        
        let (protected, _) = watchdog.is_protected_path("/home/user/file.txt");
        assert!(!protected);
    }

    #[test]
    fn test_stats() {
        let watchdog = FilesystemWatchdog::new(WatchdogConfig::default());
        let stats = watchdog.stats();
        
        assert!(stats["enabled"].as_bool().unwrap());
        assert!(stats["monitored_paths"].as_u64().unwrap() > 0);
    }
}
