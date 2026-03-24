//! Session Manager - Complete Session Lifecycle Management
//!
//! Implements: mem_session_start, mem_session_end, mem_session_summary

use crate::infrastructure::database::Database;
use anyhow::Result;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::session_context::SessionContextManager;

/// Session Manager for Synapsis
pub struct SessionManager {
    db: Arc<Database>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub project: String,
    pub directory: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub summary: Option<String>,
    pub observation_count: i32,
}

impl SessionManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// mem_session_start - Start a new session
    pub fn start_session(&self, project: &str, directory: &str) -> Result<String> {
        let session_id = format!("{}-{}", project, self.current_timestamp());

        let conn = self.db.get_conn();

        conn.execute(
            "INSERT INTO sessions (id, project, directory, started_at, observation_count)
             VALUES (?1, ?2, ?3, ?4, 0)",
            [
                &session_id,
                project,
                directory,
                &self.current_timestamp().to_string(),
            ],
        )?;

        eprintln!("[Session] Started: {} (project: {})", session_id, project);
        Ok(session_id)
    }

    /// mem_session_end - End a session with optional summary
    pub fn end_session(&self, session_id: &str, summary: Option<&str>) -> Result<()> {
        let conn = self.db.get_conn();

        conn.execute(
            "UPDATE sessions
             SET ended_at = ?1, summary = ?2
             WHERE id = ?3",
            [
                self.current_timestamp().to_string(),
                summary.unwrap_or("").to_string(),
                session_id.to_string(),
            ],
        )?;

        eprintln!("[Session] Ended: {} with summary", session_id);
        Ok(())
    }

    /// mem_session_summary - Generate and save session summary
    pub fn generate_summary(&self, session_id: &str) -> Result<String> {
        let conn = self.db.get_conn();

        // Count observations
        let obs_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM observations WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )?;

        // Get session info
        let (project, directory, started_at): (String, String, i64) = conn.query_row(
            "SELECT project, directory, started_at FROM sessions WHERE id = ?1",
            [session_id],
            |row: &rusqlite::Row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        // Generate summary
        let summary = format!(
            "Session Summary: {}\n\
             Project: {}\n\
             Directory: {}\n\
             Duration: {} seconds\n\
             Observations: {}\n\
             Status: Completed",
            session_id,
            project,
            directory,
            self.current_timestamp() - started_at,
            obs_count
        );

        // Save summary
        conn.execute(
            "UPDATE sessions SET summary = ?1 WHERE id = ?2",
            [&summary, session_id],
        )?;

        Ok(summary)
    }

    /// Get session info
    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionInfo>> {
        let conn = self.db.get_conn();

        let session = conn
            .query_row(
                "SELECT id, project, directory, started_at, ended_at, summary, observation_count
             FROM sessions WHERE id = ?1",
                [session_id],
                |row: &rusqlite::Row| {
                    Ok(SessionInfo {
                        session_id: row.get(0)?,
                        project: row.get(1)?,
                        directory: row.get(2)?,
                        started_at: row.get(3)?,
                        ended_at: row.get(4)?,
                        summary: row.get(5)?,
                        observation_count: row.get(6)?,
                    })
                },
            )
            .optional()?;

        Ok(session)
    }

    /// List recent sessions
    pub fn list_sessions(&self, limit: i32) -> Result<Vec<SessionInfo>> {
        let conn = self.db.get_conn();

        let mut stmt = conn.prepare(
            "SELECT id, project, directory, started_at, ended_at, summary, observation_count
             FROM sessions ORDER BY started_at DESC LIMIT ?1",
        )?;

        let sessions = stmt.query_map([limit], |row: &rusqlite::Row| {
            Ok(SessionInfo {
                session_id: row.get(0)?,
                project: row.get(1)?,
                directory: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                summary: row.get(5)?,
                observation_count: row.get(6)?,
            })
        })?;

        Ok(sessions
            .filter_map(|r: Result<SessionInfo, rusqlite::Error>| r.ok())
            .collect())
    }

    /// Get a SessionContextManager for persisting conversation context
    pub fn context_manager(&self) -> SessionContextManager {
        SessionContextManager::new(self.db.clone())
    }

    /// Clean up stale sessions (without ended_at) older than timeout_secs
    pub fn cleanup_stale_sessions(&self, timeout_secs: i64) -> Result<()> {
        let conn = self.db.get_conn();
        let threshold = self.current_timestamp() - timeout_secs;
        conn.execute(
            "UPDATE sessions SET ended_at = ?1 WHERE ended_at IS NULL AND started_at < ?2",
            [threshold, threshold],
        )?;
        Ok(())
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

    #[test]
    fn test_session_lifecycle() {
        // Test would require actual DB connection
        assert!(true);
    }
}
