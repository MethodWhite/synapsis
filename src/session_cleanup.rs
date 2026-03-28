//! Session Cleanup Module - Automatic Session Lifecycle Management
//!
//! This module provides automatic session cleanup functionality:
//! - Heartbeat monitoring
//! - Stale session detection
//! - Automatic cleanup of zombie sessions
//! - Resource release (locks, tasks)
//!
//! # Quick Start
//!
//! ```rust
//! // In your application initialization
//! use synapsis::session_cleanup::init_session_cleanup;
//!
//! let db = Arc::new(Database::new());
//! init_session_cleanup(&db);
//! ```

use crate::core::session_cleanup::{SessionCleanupConfig, SessionCleanupJob};
use crate::infrastructure::database::Database;
use std::sync::Arc;

/// Default session timeout (5 minutes)
pub const DEFAULT_SESSION_TIMEOUT_SECS: u64 = 300;

/// Default cleanup interval (1 minute)
pub const DEFAULT_CLEANUP_INTERVAL_SECS: u64 = 60;

/// Initialize session cleanup system with default configuration
///
/// This starts a background job that:
/// - Runs every 60 seconds
/// - Cleans sessions without heartbeat for 5+ minutes
/// - Automatically ends stale sessions
/// - Releases locks and cancels tasks
///
/// # Arguments
///
/// * `db` - Database connection
///
/// # Returns
///
/// * `Arc<SessionCleanupJob>` - Handle to control the cleanup job
///
/// # Example
///
/// ```rust
/// let db = Arc::new(Database::new());
/// let cleanup_job = init_session_cleanup(&db);
///
/// // Job is now running in background
/// assert!(cleanup_job.is_running());
///
/// // Stop on shutdown
/// // cleanup_job.stop();
/// ```
pub fn init_session_cleanup(db: &Arc<Database>) -> Arc<SessionCleanupJob> {
    init_session_cleanup_with_config(db, SessionCleanupConfig::default())
}

/// Initialize session cleanup system with custom configuration
///
/// # Arguments
///
/// * `db` - Database connection
/// * `config` - Custom cleanup configuration
///
/// # Returns
///
/// * `Arc<SessionCleanupJob>` - Handle to control the cleanup job
///
/// # Example
///
/// ```rust
/// let config = SessionCleanupConfig {
///     session_timeout_secs: 600,      // 10 minutes
///     cleanup_interval_secs: 120,     // 2 minutes
///     require_heartbeat: true,
///     auto_end_sessions: true,
/// };
///
/// let cleanup_job = init_session_cleanup_with_config(&db, config);
/// ```
pub fn init_session_cleanup_with_config(
    db: &Arc<Database>,
    config: SessionCleanupConfig,
) -> Arc<SessionCleanupJob> {
    let cleanup_job = Arc::new(SessionCleanupJob::new(db.clone(), config));

    // Start the background job
    cleanup_job.start();

    eprintln!(
        "[SessionCleanup] Initialized: running={}, timeout={}s, interval={}s",
        cleanup_job.is_running(),
        DEFAULT_SESSION_TIMEOUT_SECS,
        DEFAULT_CLEANUP_INTERVAL_SECS
    );

    cleanup_job
}

/// Manually trigger session cleanup (for CLI commands or emergency cleanup)
///
/// # Arguments
///
/// * `db` - Database connection
/// * `timeout_secs` - Session timeout threshold (0 for immediate cleanup)
///
/// # Returns
///
/// * `Result<CleanupStats, String>` - Cleanup statistics or error
///
/// # Example
///
/// ```rust
/// // Emergency cleanup (timeout=0)
/// let stats = manual_cleanup(&db, 0)?;
/// println!("Cleaned {} sessions", stats.cleaned);
///
/// // Normal cleanup (5 min timeout)
/// let stats = manual_cleanup(&db, 300)?;
/// ```
pub fn manual_cleanup(
    db: &Arc<Database>,
    timeout_secs: u64,
) -> Result<crate::core::session_cleanup::CleanupStats, String> {
    let config = SessionCleanupConfig {
        session_timeout_secs: timeout_secs,
        cleanup_interval_secs: 60,
        require_heartbeat: true,
        auto_end_sessions: true,
    };

    let cleanup_job = SessionCleanupJob::new(db.clone(), config);

    // Run cleanup once (blocking)
    futures::executor::block_on(cleanup_job.run_once())
}

/// Get session cleanup status
///
/// # Arguments
///
/// * `db` - Database connection
///
/// # Returns
///
/// * `SessionCleanupStatus` - Current cleanup system status
pub fn get_cleanup_status(db: &Arc<Database>) -> SessionCleanupStatus {
    use rusqlite::OptionalExtension;

    let conn = db.get_conn();

    // Count active sessions
    let active_sessions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE is_active = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Count stale sessions (no heartbeat for 5+ minutes)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let threshold = now - DEFAULT_SESSION_TIMEOUT_SECS as i64;

    let stale_sessions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM agent_sessions 
         WHERE is_active = 1 
         AND (last_heartbeat IS NULL OR last_heartbeat < ?1)",
            [threshold],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Count held locks
    let held_locks: i64 = conn
        .query_row("SELECT COUNT(*) FROM active_locks", [], |row| row.get(0))
        .unwrap_or(0);

    // Count pending tasks
    let pending_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_queue WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    SessionCleanupStatus {
        active_sessions,
        stale_sessions,
        held_locks,
        pending_tasks,
    }
}

/// Session cleanup system status
#[derive(Debug, Clone)]
pub struct SessionCleanupStatus {
    /// Number of active agent sessions
    pub active_sessions: i64,
    /// Number of stale sessions (no heartbeat)
    pub stale_sessions: i64,
    /// Number of held locks
    pub held_locks: i64,
    /// Number of pending tasks
    pub pending_tasks: i64,
}

impl std::fmt::Display for SessionCleanupStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Session Cleanup Status:")?;
        writeln!(f, "  Active sessions: {}", self.active_sessions)?;
        writeln!(f, "  Stale sessions:  {}", self.stale_sessions)?;
        writeln!(f, "  Held locks:      {}", self.held_locks)?;
        write!(f, "  Pending tasks:   {}", self.pending_tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_SESSION_TIMEOUT_SECS, 300);
        assert_eq!(DEFAULT_CLEANUP_INTERVAL_SECS, 60);
    }

    #[test]
    fn test_status_display() {
        let status = SessionCleanupStatus {
            active_sessions: 5,
            stale_sessions: 2,
            held_locks: 3,
            pending_tasks: 10,
        };

        let display = format!("{}", status);
        assert!(display.contains("Active sessions: 5"));
        assert!(display.contains("Stale sessions:  2")); // Note: two spaces for alignment
        assert!(display.contains("Held locks:      3"));
        assert!(display.contains("Pending tasks:   10"));
    }
}
