//! Anti-tampering detection and self-healing capabilities

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::core::integrity;

/// Tamper detection result
#[derive(Debug, Clone)]
pub enum TamperStatus {
    Valid,
    Modified { expected_hash: String, actual_hash: String },
    Missing,
    Error(String),
}

/// File integrity monitor
pub struct IntegrityMonitor {
    baseline: HashMap<PathBuf, String>, // path -> SHA3-512 hash
    alert_threshold: usize,
}

impl IntegrityMonitor {
    /// Create a new integrity monitor with empty baseline
    pub fn new() -> Self {
        Self {
            baseline: HashMap::new(),
            alert_threshold: 5,
        }
    }
    
    /// Add a file to baseline monitoring
    pub fn add_file(&mut self, path: &Path) -> io::Result<()> {
        let hash = self.compute_file_hash(path)?;
        self.baseline.insert(path.to_path_buf(), hash);
        Ok(())
    }
    
    /// Add all files in directory (recursive)
    pub fn add_directory(&mut self, dir: &Path) -> io::Result<usize> {
        let mut count = 0;
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    count += self.add_directory(&path)?;
                } else {
                    self.add_file(&path)?;
                    count += 1;
                }
            }
        }
        Ok(count)
    }
    
    /// Check all monitored files for tampering
    pub fn check_all(&self) -> Vec<(PathBuf, TamperStatus)> {
        let mut results = Vec::new();
        for (path, expected_hash) in &self.baseline {
            let status = self.check_file(path, expected_hash);
            results.push((path.clone(), status));
        }
        results
    }
    
    /// Check a specific file
    pub fn check_file(&self, path: &Path, expected_hash: &str) -> TamperStatus {
        match fs::metadata(path) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return TamperStatus::Error("Not a regular file".to_string());
                }
                
                match self.compute_file_hash(path) {
                    Ok(actual_hash) => {
                        if &actual_hash == expected_hash {
                            TamperStatus::Valid
                        } else {
                            TamperStatus::Modified {
                                expected_hash: expected_hash.to_string(),
                                actual_hash,
                            }
                        }
                    }
                    Err(e) => TamperStatus::Error(format!("Hash computation failed: {}", e)),
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => TamperStatus::Missing,
            Err(e) => TamperStatus::Error(format!("Metadata error: {}", e)),
        }
    }
    
    /// Compute SHA3-512 hash of file
    fn compute_file_hash(&self, path: &Path) -> io::Result<String> {
        use sha3::{Sha3_512, Digest};
        
        let mut file = fs::File::open(path)?;
        let mut hasher = Sha3_512::new();
        io::copy(&mut file, &mut hasher)?;
        let result = hasher.finalize();
        Ok(hex::encode(result))
    }
    
    /// Save baseline to file
    pub fn save_baseline(&self, path: &Path) -> io::Result<()> {
        let mut contents = String::new();
        for (file_path, hash) in &self.baseline {
            contents.push_str(&format!("{}|{}\n", file_path.display(), hash));
        }
        fs::write(path, contents)
    }
    
    /// Load baseline from file
    pub fn load_baseline(&mut self, path: &Path) -> io::Result<()> {
        let contents = fs::read_to_string(path)?;
        self.baseline.clear();
        
        for line in contents.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 2 {
                let file_path = PathBuf::from(parts[0]);
                let hash = parts[1].to_string();
                self.baseline.insert(file_path, hash);
            }
        }
        Ok(())
    }
}

/// Self-healing manager for automatic recovery
pub struct SelfHealingManager {
    backup_dir: PathBuf,
    max_backups: usize,
}

impl SelfHealingManager {
    /// Create a new self-healing manager
    pub fn new(backup_dir: &Path) -> Self {
        Self {
            backup_dir: backup_dir.to_path_buf(),
            max_backups: 10,
        }
    }
    
    /// Create a backup of a file
    pub fn create_backup(&self, file_path: &Path) -> io::Result<PathBuf> {
        if !self.backup_dir.exists() {
            fs::create_dir_all(&self.backup_dir)?;
        }
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let file_name = file_path.file_name()
            .unwrap_or_default()
            .to_string_lossy();
        
        let backup_name = format!("{}.backup.{}", file_name, timestamp);
        let backup_path = self.backup_dir.join(backup_name);
        
        fs::copy(file_path, &backup_path)?;
        
        // Clean up old backups
        self.cleanup_old_backups(&file_name)?;
        
        Ok(backup_path)
    }
    
    /// Restore file from latest backup
    pub fn restore_from_backup(&self, file_path: &Path) -> io::Result<()> {
        let file_name = file_path.file_name()
            .unwrap_or_default()
            .to_string_lossy();
        
        let mut backups: Vec<PathBuf> = fs::read_dir(&self.backup_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with(&format!("{}.backup.", file_name)))
                    .unwrap_or(false)
            })
            .collect();
        
        backups.sort();
        
        if let Some(latest_backup) = backups.last() {
            fs::copy(latest_backup, file_path)?;
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "No backup found"))
        }
    }
    
    /// Clean up old backups beyond max_backups limit
    fn cleanup_old_backups(&self, file_name: &str) -> io::Result<()> {
        let mut backups: Vec<PathBuf> = fs::read_dir(&self.backup_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with(&format!("{}.backup.", file_name)))
                    .unwrap_or(false)
            })
            .collect();
        
        backups.sort();
        
        if backups.len() > self.max_backups {
            let to_delete = backups.len() - self.max_backups;
            for backup in backups.iter().take(to_delete) {
                fs::remove_file(backup)?;
            }
        }
        
        Ok(())
    }
    
    /// Automated healing workflow
    pub fn heal_file(&self, file_path: &Path, monitor: &IntegrityMonitor) -> io::Result<bool> {
        // Check if file is in baseline
        let expected_hash = match monitor.baseline.get(file_path) {
            Some(hash) => hash,
            None => return Ok(false), // Not monitored
        };
        
        match monitor.check_file(file_path, expected_hash) {
            TamperStatus::Valid => Ok(false), // No healing needed
            TamperStatus::Modified { .. } | TamperStatus::Missing => {
                // Try to restore from backup
                match self.restore_from_backup(file_path) {
                    Ok(_) => Ok(true),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        // No backup exists, report failure
                        Err(io::Error::new(io::ErrorKind::NotFound, 
                            format!("Cannot heal {}: no backup available", file_path.display())))
                    }
                    Err(e) => Err(e),
                }
            }
            TamperStatus::Error(e) => {
                Err(io::Error::new(io::ErrorKind::Other, 
                    format!("Cannot check file {}: {}", file_path.display(), e)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    
    #[test]
    fn test_integrity_monitor() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        
        // Create test file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Test content").unwrap();
        
        // Add to monitor
        let mut monitor = IntegrityMonitor::new();
        monitor.add_file(&file_path).unwrap();
        
        // Check - should be valid
        let results = monitor.check_all();
        assert_eq!(results.len(), 1);
        match &results[0].1 {
            TamperStatus::Valid => (),
            _ => panic!("File should be valid"),
        }
        
        // Tamper file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Modified content").unwrap();
        
        // Check again - should detect modification
        let results = monitor.check_all();
        match &results[0].1 {
            TamperStatus::Modified { expected_hash, actual_hash } => {
                assert_ne!(expected_hash, actual_hash);
            }
            _ => panic!("Should detect modification"),
        }
        
        // Delete file
        fs::remove_file(&file_path).unwrap();
        
        // Check - should detect missing
        let results = monitor.check_all();
        match &results[0].1 {
            TamperStatus::Missing => (),
            _ => panic!("Should detect missing file"),
        }
    }
    
    #[test]
    fn test_self_healing() {
        let dir = tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        let file_path = dir.path().join("data.txt");
        
        // Create initial file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Original content").unwrap();
        
        // Create monitor and add file
        let mut monitor = IntegrityMonitor::new();
        monitor.add_file(&file_path).unwrap();
        
        // Create self-healing manager
        let healer = SelfHealingManager::new(&backup_dir);
        
        // Create backup
        let backup_path = healer.create_backup(&file_path).unwrap();
        assert!(backup_path.exists());
        
        // Tamper file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Tampered content").unwrap();
        
        // Verify tampering detected
        let expected_hash = monitor.baseline.get(&file_path).unwrap();
        match monitor.check_file(&file_path, expected_hash) {
            TamperStatus::Modified { .. } => (),
            _ => panic!("Should detect tampering"),
        }
        
        // Restore from backup
        healer.restore_from_backup(&file_path).unwrap();
        
        // Verify restored content
        let restored_content = fs::read_to_string(&file_path).unwrap();
        assert!(restored_content.contains("Original content"));
        
        // Verify integrity restored
        match monitor.check_file(&file_path, expected_hash) {
            TamperStatus::Valid => (),
            _ => panic!("Should be valid after restore"),
        }
    }
    
    #[test]
    fn test_heal_file_workflow() {
        let dir = tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        let file_path = dir.path().join("config.txt");
        
        // Create initial file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Config data").unwrap();
        
        // Setup monitor and healer
        let mut monitor = IntegrityMonitor::new();
        monitor.add_file(&file_path).unwrap();
        
        let healer = SelfHealingManager::new(&backup_dir);
        
        // Create backup
        healer.create_backup(&file_path).unwrap();
        
        // Tamper file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Malicious config").unwrap();
        
        // Heal file
        let healed = healer.heal_file(&file_path, &monitor).unwrap();
        assert!(healed, "File should be healed");
        
        // Verify healed content
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("Config data"));
    }
}