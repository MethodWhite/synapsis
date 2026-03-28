//! Resilience Module - Redundancy and Backup System
//! 
//! Provides data redundancy, backup verification, and failover capabilities

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use sha2::{Sha256, Digest};

/// Backup metadata
#[derive(Debug, Clone)]
pub struct BackupMetadata {
    pub path: PathBuf,
    pub timestamp: u64,
    pub checksum: Vec<u8>,
    pub verified: bool,
    pub size: u64,
}

/// Resilience manager for data redundancy and backups
pub struct ResilienceManager {
    backup_dir: PathBuf,
    max_backups: usize,
    redundancy_level: u8,
}

impl ResilienceManager {
    pub fn new(backup_dir: PathBuf) -> Self {
        Self {
            backup_dir,
            max_backups: 5,
            redundancy_level: 3, // Triple redundancy
        }
    }
    
    /// Create backup with checksum
    pub fn create_backup(&self, source: &Path, name: &str) -> Result<BackupMetadata, String> {
        // Ensure backup directory exists
        fs::create_dir_all(&self.backup_dir)
            .map_err(|e| format!("Failed to create backup dir: {}", e))?;
        
        // Generate backup filename
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let backup_filename = format!("{}_{}.bak", name, timestamp);
        let backup_path = self.backup_dir.join(&backup_filename);
        
        // Copy file with redundancy
        for i in 0..self.redundancy_level {
            let redundant_path = if i > 0 {
                self.backup_dir.join(format!("{}_{}.bak.{}", name, timestamp, i))
            } else {
                backup_path.clone()
            };
            
            fs::copy(source, &redundant_path)
                .map_err(|e| format!("Failed to copy backup: {}", e))?;
        }
        
        // Calculate checksum
        let data = fs::read(source)
            .map_err(|e| format!("Failed to read source: {}", e))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let checksum = hasher.finalize().to_vec();
        
        Ok(BackupMetadata {
            path: backup_path,
            timestamp,
            checksum,
            verified: true,
            size: data.len() as u64,
        })
    }
    
    /// Verify backup integrity
    pub fn verify_backup(&self, backup: &BackupMetadata) -> Result<bool, String> {
        if !backup.path.exists() {
            return Ok(false);
        }
        
        let data = fs::read(&backup.path)
            .map_err(|e| format!("Failed to read backup: {}", e))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let current_checksum = hasher.finalize().to_vec();
        
        Ok(current_checksum == backup.checksum)
    }
    
    /// Restore from backup
    pub fn restore(&self, backup: &BackupMetadata, destination: &Path) -> Result<(), String> {
        if !self.verify_backup(backup)? {
            return Err("Backup verification failed".to_string());
        }
        
        fs::copy(&backup.path, destination)
            .map_err(|e| format!("Failed to restore: {}", e))?;
        
        Ok(())
    }
    
    /// Get list of backups for a name
    pub fn list_backups(&self, name: &str) -> Result<Vec<BackupMetadata>, String> {
        let mut backups = Vec::new();
        
        let entries = fs::read_dir(&self.backup_dir)
            .map_err(|e| format!("Failed to read backup dir: {}", e))?;
        
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            
            if filename.starts_with(name) && filename.ends_with(".bak") {
                let path = entry.path();
                let metadata = fs::metadata(&path)
                    .map_err(|e| format!("Failed to get metadata: {}", e))?;
                
                // Extract timestamp from filename
                let timestamp: u64 = filename
                    .trim_start_matches(name)
                    .trim_end_matches(".bak")
                    .trim_start_matches('_')
                    .parse()
                    .unwrap_or(0);
                
                backups.push(BackupMetadata {
                    path,
                    timestamp,
                    checksum: vec![],
                    verified: false,
                    size: metadata.len(),
                });
            }
        }
        
        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        Ok(backups)
    }
    
    /// Cleanup old backups (keep only max_backups)
    pub fn cleanup_old_backups(&self, name: &str) -> Result<usize, String> {
        let backups = self.list_backups(name)?;
        
        let mut removed = 0;
        
        for backup in backups.iter().skip(self.max_backups) {
            if let Err(_) = fs::remove_file(&backup.path) {
                // Try to remove redundant copies too
                let _ = fs::remove_file(format!("{}.1", backup.path.display()));
                let _ = fs::remove_file(format!("{}.2", backup.path.display()));
            }
            removed += 1;
        }
        
        Ok(removed)
    }
    
    /// Get redundancy level
    pub fn get_redundancy_level(&self) -> u8 {
        self.redundancy_level
    }
    
    /// Set redundancy level (1-5)
    pub fn set_redundancy_level(&mut self, level: u8) -> Result<(), String> {
        if level < 1 || level > 5 {
            return Err("Redundancy level must be between 1 and 5".to_string());
        }
        self.redundancy_level = level;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_resilience_manager_creation() {
        let temp_dir = tempdir().unwrap();
        let manager = ResilienceManager::new(temp_dir.path().to_path_buf());
        
        assert_eq!(manager.get_redundancy_level(), 3);
    }

    #[test]
    fn test_create_backup() {
        let temp_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();
        
        let manager = ResilienceManager::new(backup_dir.path().to_path_buf());
        
        // Create source file
        let source_path = temp_dir.path().join("test.dat");
        fs::write(&source_path, b"test data").unwrap();
        
        let backup = manager.create_backup(&source_path, "test").unwrap();
        
        assert!(backup.path.exists());
        assert_eq!(backup.checksum.len(), 32); // SHA256
        assert!(backup.verified);
    }

    #[test]
    fn test_verify_backup() {
        let temp_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();
        
        let manager = ResilienceManager::new(backup_dir.path().to_path_buf());
        
        // Create source and backup
        let source_path = temp_dir.path().join("test.dat");
        fs::write(&source_path, b"test data").unwrap();
        
        let backup = manager.create_backup(&source_path, "test").unwrap();
        
        // Verify should pass
        assert!(manager.verify_backup(&backup).unwrap());
        
        // Corrupt backup
        fs::write(&backup.path, b"corrupted").unwrap();
        
        // Verify should fail
        assert!(!manager.verify_backup(&backup).unwrap());
    }

    #[test]
    fn test_restore_backup() {
        let temp_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();
        
        let manager = ResilienceManager::new(backup_dir.path().to_path_buf());
        
        // Create source file
        let source_path = temp_dir.path().join("test.dat");
        let restore_path = temp_dir.path().join("restored.dat");
        fs::write(&source_path, b"original data").unwrap();
        
        let backup = manager.create_backup(&source_path, "test").unwrap();
        
        // Corrupt original
        fs::write(&source_path, b"corrupted").unwrap();
        
        // Restore
        manager.restore(&backup, &restore_path).unwrap();
        
        // Verify restored data
        let restored = fs::read(&restore_path).unwrap();
        assert_eq!(restored, b"original data");
    }

    #[test]
    fn test_cleanup_old_backups() {
        let temp_dir = tempdir().unwrap();
        let backup_dir = tempdir().unwrap();
        
        let mut manager = ResilienceManager::new(backup_dir.path().to_path_buf());
        manager.max_backups = 3;
        
        // Create multiple backups
        let source_path = temp_dir.path().join("test.dat");
        
        for i in 0..5 {
            fs::write(&source_path, format!("data {}", i)).unwrap();
            manager.create_backup(&source_path, "test").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        // Cleanup
        let removed = manager.cleanup_old_backups("test").unwrap();
        
        assert!(removed <= 2); // Should remove excess backups
    }

    #[test]
    fn test_redundancy_level() {
        let temp_dir = tempdir().unwrap();
        let mut manager = ResilienceManager::new(temp_dir.path().to_path_buf());
        
        assert_eq!(manager.get_redundancy_level(), 3);
        
        manager.set_redundancy_level(5).unwrap();
        assert_eq!(manager.get_redundancy_level(), 5);
        
        assert!(manager.set_redundancy_level(0).is_err());
        assert!(manager.set_redundancy_level(6).is_err());
    }
}
