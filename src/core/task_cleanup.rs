//! Task Cleanup System
//!
//! Automatically removes stale/obsolete tasks from projects.
//! Keeps task queue clean and efficient.

use crate::infrastructure::database::Database;
use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};

/// Task Cleanup Configuration
#[derive(Debug, Clone)]
pub struct CleanupConfig {
    /// Max age in seconds before task is considered stale
    pub max_age_secs: i64,

    /// Max completed tasks to keep per project
    pub max_completed_per_project: usize,

    /// Auto-remove failed tasks older than this
    pub failed_task_ttl_secs: i64,

    /// Enable automatic cleanup on startup
    pub auto_cleanup_on_startup: bool,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            max_age_secs: 86400, // 24 hours
            max_completed_per_project: 10,
            failed_task_ttl_secs: 3600, // 1 hour
            auto_cleanup_on_startup: true,
        }
    }
}

/// Task Cleanup Manager
pub struct TaskCleanupManager {
    db: std::sync::Arc<Database>,
    config: CleanupConfig,
}

impl TaskCleanupManager {
    pub fn new(db: std::sync::Arc<Database>) -> Self {
        Self {
            db,
            config: CleanupConfig::default(),
        }
    }

    pub fn with_config(db: std::sync::Arc<Database>, config: CleanupConfig) -> Self {
        Self { db, config }
    }

    /// Run cleanup on all projects
    pub fn run_cleanup(&self) -> Result<CleanupReport> {
        let mut report = CleanupReport::default();

        // Remove stale pending tasks
        report.removed_stale = self.remove_stale_pending_tasks()?;

        // Remove old failed tasks
        report.removed_failed = self.remove_old_failed_tasks()?;

        // Archive old completed tasks
        report.archived_completed = self.archive_old_completed_tasks()?;

        // Compact database
        report.vacuumed = self.vacuum_database()?;

        Ok(report)
    }

    /// Remove pending tasks older than max_age_secs
    fn remove_stale_pending_tasks(&self) -> Result<usize> {
        let conn = self.db.get_conn();
        let cutoff = self.current_timestamp() - self.config.max_age_secs;

        let mut stmt = conn.prepare_cached(
            "DELETE FROM task_queue
             WHERE status = 'pending'
             AND created_at < ?1"
        )?;

        let removed = stmt.execute([cutoff])?;

        if removed > 0 {
            eprintln!("[TaskCleanup] Removed {} stale pending tasks", removed);
        }

        Ok(removed as usize)
    }

    /// Remove failed tasks older than failed_task_ttl_secs
    fn remove_old_failed_tasks(&self) -> Result<usize> {
        let conn = self.db.get_conn();
        let cutoff = self.current_timestamp() - self.config.failed_task_ttl_secs;

        let mut stmt = conn.prepare_cached(
            "DELETE FROM task_queue
             WHERE status = 'failed'
             AND updated_at < ?1"
        )?;

        let removed = stmt.execute([cutoff])?;

        if removed > 0 {
            eprintln!("[TaskCleanup] Removed {} old failed tasks", removed);
        }

        Ok(removed as usize)
    }

    /// Archive old completed tasks (keep only recent ones)
    fn archive_old_completed_tasks(&self) -> Result<usize> {
        let conn = self.db.get_conn();

        // Get all projects
        let mut projects_stmt = conn.prepare_cached(
            "SELECT DISTINCT project_key FROM task_queue WHERE status = 'completed'"
        )?;

        let projects: Vec<String> = projects_stmt
            .query_map([], |row: &rusqlite::Row| row.get(0))?
            .filter_map(|r: Result<String, rusqlite::Error>| r.ok())
            .collect();

        let mut archived = 0;

        for project in projects {
            // Keep only max_completed_per_project most recent
            let mut stmt = conn.prepare_cached(
                "DELETE FROM task_queue
                 WHERE status = 'completed'
                 AND project_key = ?1
                 AND rowid NOT IN (
                     SELECT rowid FROM task_queue
                     WHERE status = 'completed'
                     AND project_key = ?1
                     ORDER BY updated_at DESC
                     LIMIT ?2
                 )"
            )?;

            let removed = stmt.execute([&project, &(self.config.max_completed_per_project as i64).to_string()])?;
            archived += removed;
        }

        if archived > 0 {
            eprintln!("[TaskCleanup] Archived {} old completed tasks", archived);
        }

        Ok(archived as usize)
    }

    /// Vacuum database to reclaim space
    fn vacuum_database(&self) -> Result<bool> {
        let conn = self.db.get_conn();
        conn.execute("VACUUM", [])?;
        eprintln!("[TaskCleanup] Database vacuumed");
        Ok(true)
    }

    fn current_timestamp(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

/// Cleanup Report
#[derive(Debug, Default)]
pub struct CleanupReport {
    pub removed_stale: usize,
    pub removed_failed: usize,
    pub archived_completed: usize,
    pub vacuumed: bool,
}

impl CleanupReport {
    pub fn total_removed(&self) -> usize {
        self.removed_stale + self.removed_failed + self.archived_completed
    }

    pub fn summary(&self) -> String {
        format!(
            "Cleanup complete: {} removed ({} stale, {} failed, {} archived), vacuum: {}",
            self.total_removed(),
            self.removed_stale,
            self.removed_failed,
            self.archived_completed,
            if self.vacuumed { "yes" } else { "no" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_config_defaults() {
        let config = CleanupConfig::default();
        assert_eq!(config.max_age_secs, 86400);
        assert_eq!(config.max_completed_per_project, 10);
        assert_eq!(config.failed_task_ttl_secs, 3600);
        assert!(config.auto_cleanup_on_startup);
    }

    #[test]
    fn test_cleanup_report() {
        let report = CleanupReport {
            removed_stale: 5,
            removed_failed: 3,
            archived_completed: 10,
            vacuumed: true,
        };

        assert_eq!(report.total_removed(), 18);
        assert!(report.summary().contains("18 removed"));
    }
}
