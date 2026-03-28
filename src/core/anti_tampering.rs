//! Anti-Tampering Module - File Integrity Monitoring
//! 
//! Provides file integrity monitoring, tamper detection, and automatic alerts

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};
use sha2::Sha256 as Sha256Hash;

type HmacSha256 = Hmac<Sha256Hash>;

/// File integrity record
#[derive(Debug, Clone)]
pub struct IntegrityRecord {
    pub path: PathBuf,
    pub checksum: Vec<u8>,
    pub hmac: Vec<u8>,
    pub last_verified: Instant,
    pub tampered: bool,
}

/// Tamper alert
#[derive(Debug, Clone)]
pub struct TamperAlert {
    pub path: PathBuf,
    pub timestamp: u64,
    pub expected_checksum: Vec<u8>,
    pub actual_checksum: Vec<u8>,
    pub severity: AlertSeverity,
}

/// Alert severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Anti-tampering manager
pub struct AntiTamperingManager {
    monitored_files: HashMap<PathBuf, IntegrityRecord>,
    secret_key: Vec<u8>,
    alert_callback: Option<Box<dyn Fn(TamperAlert) + Send + Sync>>,
}

impl AntiTamperingManager {
    pub fn new(secret_key: &[u8]) -> Self {
        Self {
            monitored_files: HashMap::new(),
            secret_key: secret_key.to_vec(),
            alert_callback: None,
        }
    }
    
    /// Set alert callback
    pub fn set_alert_callback<F>(&mut self, callback: F)
    where
        F: Fn(TamperAlert) + Send + Sync + 'static
    {
        self.alert_callback = Some(Box::new(callback));
    }
    
    /// Add file to monitoring
    pub fn monitor_file(&mut self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("File does not exist: {:?}", path));
        }
        
        let data = fs::read(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        
        // Calculate checksum
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let checksum = hasher.finalize().to_vec();
        
        // Calculate HMAC
        let mut mac = HmacSha256::new_from_slice(&self.secret_key)
            .map_err(|e| format!("HMAC error: {}", e))?;
        mac.update(&data);
        let hmac = mac.finalize().into_bytes().to_vec();
        
        let record = IntegrityRecord {
            path: path.to_path_buf(),
            checksum,
            hmac,
            last_verified: Instant::now(),
            tampered: false,
        };
        
        self.monitored_files.insert(path.to_path_buf(), record);
        
        Ok(())
    }
    
    /// Verify file integrity
    pub fn verify_file(&mut self, path: &Path) -> Result<bool, String> {
        let record = self.monitored_files.get_mut(path)
            .ok_or_else(|| format!("File not monitored: {:?}", path))?;
        
        if !record.path.exists() {
            record.tampered = true;
            self.send_alert(path, &record.checksum, &vec![], AlertSeverity::Critical);
            return Ok(false);
        }
        
        let data = fs::read(&record.path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        
        // Verify checksum
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let current_checksum = hasher.finalize().to_vec();
        
        if current_checksum != record.checksum {
            record.tampered = true;
            self.send_alert(path, &record.checksum, &current_checksum, AlertSeverity::Critical);
            return Ok(false);
        }
        
        // Verify HMAC
        let mut mac = HmacSha256::new_from_slice(&self.secret_key).unwrap();
        mac.update(&data);
        
        if mac.verify_slice(&record.hmac).is_err() {
            record.tampered = true;
            self.send_alert(path, &record.checksum, &current_checksum, AlertSeverity::Critical);
            return Ok(false);
        }
        
        record.last_verified = Instant::now();
        record.tampered = false;
        
        Ok(true)
    }
    
    /// Verify all monitored files
    pub fn verify_all(&mut self) -> Result<usize, String> {
        let mut tampered_count = 0;
        
        let paths: Vec<PathBuf> = self.monitored_files.keys().cloned().collect();
        
        for path in paths {
            if let Ok(is_valid) = self.verify_file(&path) {
                if !is_valid {
                    tampered_count += 1;
                }
            }
        }
        
        Ok(tampered_count)
    }
    
    /// Send tamper alert
    fn send_alert(
        &self,
        path: &Path,
        expected: &[u8],
        actual: &[u8],
        severity: AlertSeverity
    ) {
        if let Some(ref callback) = self.alert_callback {
            let alert = TamperAlert {
                path: path.to_path_buf(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                expected_checksum: expected.to_vec(),
                actual_checksum: actual.to_vec(),
                severity,
            };
            
            callback(alert);
        }
    }
    
    /// Get monitoring status
    pub fn get_status(&self) -> MonitoringStatus {
        let total = self.monitored_files.len();
        let tampered = self.monitored_files.values().filter(|r| r.tampered).count();
        let healthy = total - tampered;
        
        MonitoringStatus {
            total,
            healthy,
            tampered,
            last_check: Instant::now(),
        }
    }
    
    /// Get list of tampered files
    pub fn get_tampered_files(&self) -> Vec<&IntegrityRecord> {
        self.monitored_files.values()
            .filter(|r| r.tampered)
            .collect()
    }
}

/// Monitoring status summary
#[derive(Debug, Clone)]
pub struct MonitoringStatus {
    pub total: usize,
    pub healthy: usize,
    pub tampered: usize,
    pub last_check: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_anti_tampering_creation() {
        let manager = AntiTamperingManager::new(b"secret_key");
        let status = manager.get_status();
        
        assert_eq!(status.total, 0);
        assert_eq!(status.healthy, 0);
        assert_eq!(status.tampered, 0);
    }

    #[test]
    fn test_monitor_file() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test content").unwrap();
        
        let mut manager = AntiTamperingManager::new(b"secret_key");
        let result = manager.monitor_file(&file_path);
        
        assert!(result.is_ok());
        assert_eq!(manager.get_status().total, 1);
    }

    #[test]
    fn test_verify_untampered_file() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test content").unwrap();
        
        let mut manager = AntiTamperingManager::new(b"secret_key");
        manager.monitor_file(&file_path).unwrap();
        
        let result = manager.verify_file(&file_path).unwrap();
        assert!(result);
        
        let status = manager.get_status();
        assert_eq!(status.tampered, 0);
    }

    #[test]
    fn test_detect_tampering() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"original content").unwrap();
        
        let mut manager = AntiTamperingManager::new(b"secret_key");
        manager.monitor_file(&file_path).unwrap();
        
        // Tamper with file
        fs::write(&file_path, b"tampered content").unwrap();
        
        let result = manager.verify_file(&file_path).unwrap();
        assert!(!result);
        
        let status = manager.get_status();
        assert_eq!(status.tampered, 1);
    }

    #[test]
    fn test_detect_file_deletion() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test content").unwrap();
        
        let mut manager = AntiTamperingManager::new(b"secret_key");
        manager.monitor_file(&file_path).unwrap();
        
        // Delete file
        fs::remove_file(&file_path).unwrap();
        
        let result = manager.verify_file(&file_path).unwrap();
        assert!(!result);
        
        let tampered = manager.get_tampered_files();
        assert_eq!(tampered.len(), 1);
    }

    #[test]
    fn test_alert_callback() {
        use std::sync::{Arc, Mutex};
        
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test content").unwrap();
        
        let alerts = Arc::new(Mutex::new(Vec::new()));
        let alerts_clone = alerts.clone();
        
        let mut manager = AntiTamperingManager::new(b"secret_key");
        manager.set_alert_callback(move |alert| {
            let mut a = alerts_clone.lock().unwrap();
            a.push(alert);
        });
        
        manager.monitor_file(&file_path).unwrap();
        
        // Tamper
        fs::write(&file_path, b"tampered").unwrap();
        manager.verify_file(&file_path).unwrap();
        
        let a = alerts.lock().unwrap();
        assert!(!a.is_empty());
        assert_eq!(a[0].severity, AlertSeverity::Critical);
    }

    #[test]
    fn test_verify_all() {
        let temp_dir = tempdir().unwrap();
        
        let mut manager = AntiTamperingManager::new(b"secret_key");
        
        // Create and monitor multiple files
        for i in 0..5 {
            let file_path = temp_dir.path().join(format!("file_{}.txt", i));
            fs::write(&file_path, format!("content {}", i)).unwrap();
            manager.monitor_file(&file_path).unwrap();
        }
        
        // Tamper with one
        let tampered_path = temp_dir.path().join("file_2.txt");
        fs::write(&tampered_path, "tampered").unwrap();
        
        let tampered_count = manager.verify_all().unwrap();
        assert_eq!(tampered_count, 1);
    }
}
