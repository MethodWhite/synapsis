//! Audit Log - Track all changes for mem_update/mem_delete
//!
//! Provides audit trail for observation modifications.

use crate::infrastructure::database::Database;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Audit Log Entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub observation_id: i64,
    pub action: String,  // "update", "delete", "restore"
    pub agent_id: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub reason: Option<String>,
    pub timestamp: i64,
}

/// Audit Log Manager
pub struct AuditLog {
    db: Arc<Database>,
}

impl AuditLog {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Log an update action
    pub fn log_update(&self, obs_id: i64, agent_id: &str, old_content: &str, new_content: &str, reason: Option<&str>) -> Result<()> {
        Ok(self.db.log_audit(
            Some(obs_id),
            "update",
            agent_id,
            Some(old_content),
            Some(new_content),
            reason,
        )?)
    }

    /// Log a soft delete action
    pub fn log_delete(&self, obs_id: i64, agent_id: &str, reason: Option<&str>) -> Result<()> {
        Ok(self.db.log_audit(
            Some(obs_id),
            "delete",
            agent_id,
            None,
            None,
            reason,
        )?)
    }

    /// Log a restore action
    pub fn log_restore(&self, obs_id: i64, agent_id: &str) -> Result<()> {
        Ok(self.db.log_audit(
            Some(obs_id),
            "restore",
            agent_id,
            None,
            None,
            Some("Restored from soft delete"),
        )?)
    }

    /// Get audit trail for an observation
    pub fn get_audit_trail(&self, obs_id: i64) -> Result<Vec<AuditEntry>> {
        let json_entries = self.db.get_audit_trail(obs_id)?;
        let mut entries = Vec::new();
        for json_entry in json_entries {
            let entry = AuditEntry {
                id: json_entry["id"].as_i64().unwrap_or(0),
                observation_id: json_entry["observation_id"].as_i64().unwrap_or(0),
                action: json_entry["action"].as_str().unwrap_or("").to_string(),
                agent_id: json_entry["agent_id"].as_str().unwrap_or("").to_string(),
                old_value: json_entry["old_value"].as_str().map(String::from),
                new_value: json_entry["new_value"].as_str().map(String::from),
                reason: json_entry["reason"].as_str().map(String::from),
                timestamp: json_entry["timestamp"].as_i64().unwrap_or(0),
            };
            entries.push(entry);
        }
        Ok(entries)
    }

    fn current_timestamp(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::StoragePort;
    use crate::infrastructure::database::Database;
    use std::sync::Arc;

    #[test]
    fn test_audit_log_creation() {
        // Create an in-memory database for testing
        std::env::set_var("XDG_DATA_HOME", "/tmp/synapsis-test-audit");
        std::fs::create_dir_all("/tmp/synapsis-test-audit/synapsis").ok();
        let db = Arc::new(Database::new());
        db.init().ok();
        
        let log = AuditLog::new(db);
        assert!(log.log_update(1, "agent1", "old", "new", None).is_ok());
        
        // Cleanup
        std::fs::remove_dir_all("/tmp/synapsis-test-audit").ok();
    }
}
