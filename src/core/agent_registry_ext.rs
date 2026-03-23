//! Extended Agent Registry - Complete Agent Management
//!
//! Implements: agent_unregister, agent_list_by_project, agent_session_cleanup

use crate::infrastructure::database::Database;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Agent Info Extended
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfoExt {
    pub id: String,
    pub agent_type: String,
    pub agent_instance: String,
    pub project_key: String,
    pub pid: Option<i32>,
    pub started_at: i64,
    pub last_heartbeat: i64,
    pub is_active: bool,
    pub current_task: Option<String>,
}

/// Extended Agent Registry
pub struct AgentRegistryExt {
    db: Arc<Database>,
}

impl AgentRegistryExt {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// agent_unregister - Unregister an agent
    pub fn unregister_agent(&self, agent_id: &str) -> Result<()> {
        let conn = self.db.get_conn();

        conn.execute(
            "UPDATE agent_sessions SET is_active = 0, current_task = 'unregistered' WHERE id = ?1",
            [agent_id],
        )?;

        eprintln!("[Agent] Unregistered: {}", agent_id);
        Ok(())
    }

    /// agent_list_by_project - List agents by project
    pub fn list_agents_by_project(&self, project_key: &str) -> Result<Vec<AgentInfoExt>> {
        let conn = self.db.get_conn();

        let mut stmt = conn.prepare(
            "SELECT id, agent_type, agent_instance, project_key, pid, started_at, last_heartbeat, is_active, current_task
             FROM agent_sessions
             WHERE project_key = ?1
             ORDER BY last_heartbeat DESC",
        )?;

        let agents = stmt.query_map([project_key], |row: &rusqlite::Row| {
            Ok(AgentInfoExt {
                id: row.get(0)?,
                agent_type: row.get(1)?,
                agent_instance: row.get(2)?,
                project_key: row.get(3)?,
                pid: row.get(4)?,
                started_at: row.get(5)?,
                last_heartbeat: row.get(6)?,
                is_active: row.get(7)?,
                current_task: row.get(8)?,
            })
        })?;

        Ok(agents.filter_map(|r: Result<AgentInfoExt, rusqlite::Error>| r.ok()).collect())
    }

    /// agent_session_cleanup - Clean up stale agent sessions
    pub fn cleanup_stale_sessions(&self, max_age_secs: i64) -> Result<usize> {
        let conn = self.db.get_conn();
        let cutoff = self.current_timestamp() - max_age_secs;

        // Mark stale agents as inactive
        let modified = conn.execute(
            "UPDATE agent_sessions
             SET is_active = 0, current_task = 'stale'
             WHERE last_heartbeat < ?1 AND is_active = 1",
            [cutoff],
        )?;

        eprintln!("[Agent] Cleaned up {} stale sessions", modified);
        Ok(modified as usize)
    }

    /// Get active agents count
    pub fn get_active_count(&self) -> Result<i64> {
        let conn = self.db.get_conn();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE is_active = 1",
            [],
            |row| row.get(0),
        )?;

        Ok(count)
    }

    fn current_timestamp(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}
