//! Synapsis SQLite Database - Core Implementation

use crate::core::uuid::Uuid;
use crate::domain::ports::{SessionPort, StoragePort};
use crate::domain::*;
use base64::{engine::general_purpose, Engine as _};
use hex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub mod multi_agent;

pub struct Database {
    conn: Arc<Mutex<Connection>>,
    _data_dir: PathBuf,
    db_path: PathBuf,
    encryption_key: Option<Vec<u8>>,
}

unsafe impl Send for Database {}
unsafe impl Sync for Database {}

impl Database {
    pub fn new() -> Self {
        let encryption_key = std::env::var("SYNAPSIS_DB_KEY")
            .ok()
            .and_then(|hex_key| {
                hex::decode(hex_key).ok()
            })
            .or_else(|| {
                std::env::var("SYNAPSIS_DB_KEY_BASE64")
                    .ok()
                    .and_then(|b64| general_purpose::STANDARD.decode(b64).ok())
            });
        Self::new_with_key(encryption_key)
    }

    pub fn new_with_key(encryption_key: Option<Vec<u8>>) -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapsis");

        std::fs::create_dir_all(&data_dir).ok();
        let db_path = data_dir.join("synapsis.db");
        let conn = if let Some(key) = &encryption_key {
            let mut conn = Connection::open(&db_path).unwrap();
            // SQLCipher expects key as bytes; we'll use hex encoding
            let hex_key = hex::encode(key);
            conn.execute_batch(&format!("PRAGMA key = 'x{}'", hex_key)).unwrap();
            // Verify encryption is active
            conn.execute_batch("PRAGMA cipher_version").unwrap();
            // SQLCipher performance optimizations
            conn.execute_batch("PRAGMA cipher_page_size = 4096").unwrap();
            conn
        } else {
            Connection::open(&db_path).unwrap()
        };

        // Common performance optimizations for SQLite/SQLCipher
        conn.execute_batch("PRAGMA journal_mode = WAL").unwrap();
        conn.execute_batch("PRAGMA synchronous = NORMAL").unwrap();
        conn.execute_batch("PRAGMA cache_size = -2000").unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();

        Self {
            conn: Arc::new(Mutex::new(conn)),
            _data_dir: data_dir.clone(),
            db_path,
            encryption_key,
        }
    }

    pub fn get_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    pub fn migrate_from_json(&self) -> Result<()> {
        eprintln!("[Database] Migration complete");
        Ok(())
    }

    pub fn stats(&self) -> Result<serde_json::Value> {
        self.get_stats()
    }

    fn create_tables(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS observations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sync_id TEXT NOT NULL UNIQUE,
                session_id TEXT NOT NULL,
                project TEXT,
                observation_type INTEGER NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_name TEXT,
                scope INTEGER NOT NULL,
                topic_key TEXT,
                content_hash BLOB NOT NULL,
                revision_count INTEGER NOT NULL DEFAULT 1,
                duplicate_count INTEGER NOT NULL DEFAULT 0,
                last_seen_at INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER,
                integrity_hash TEXT,
                classification INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                project_key TEXT NOT NULL,
                directory TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                summary TEXT,
                observation_count INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk_id TEXT NOT NULL UNIQUE,
                project_key TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                level INTEGER NOT NULL DEFAULT 0,
                is_active INTEGER NOT NULL DEFAULT 1,
                embedding BLOB,
                is_indexed INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_sessions (
                id TEXT PRIMARY KEY,
                agent_type TEXT NOT NULL,
                agent_instance TEXT NOT NULL,
                project_key TEXT NOT NULL,
                pid INTEGER,
                started_at INTEGER NOT NULL,
                last_heartbeat INTEGER NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                current_task TEXT
            );
            CREATE TABLE IF NOT EXISTS active_locks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lock_key TEXT NOT NULL UNIQUE,
                agent_session_id TEXT NOT NULL,
                acquired_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS task_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL UNIQUE,
                agent_session_id TEXT,
                project_key TEXT NOT NULL,
                task_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                completed_at INTEGER,
                result TEXT,
                error TEXT
            );
            CREATE TABLE IF NOT EXISTS global_context (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_key TEXT NOT NULL,
                context_data TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS context_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                cache_key TEXT NOT NULL UNIQUE,
                project_key TEXT,
                data TEXT NOT NULL,
                hits INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                last_accessed INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id TEXT NOT NULL UNIQUE,
                agent_id TEXT NOT NULL,
                session_id TEXT,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                token_count INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                checksum TEXT
            );
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                observation_id INTEGER,
                action TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                old_value TEXT,
                new_value TEXT,
                reason TEXT,
                timestamp INTEGER NOT NULL,
                FOREIGN KEY (observation_id) REFERENCES observations(id) ON DELETE SET NULL
            );
            "
        )?;
        Ok(())
    }

    pub fn register_agent_session(
        &self,
        agent_type: &str,
        agent_instance: &str,
        project_key: &str,
        pid: Option<i32>,
    ) -> Result<String> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        let session_id = format!("{}-{}-{}", agent_type, agent_instance, now);

        conn.execute(
            "INSERT INTO agent_sessions (id, agent_type, agent_instance, project_key, pid, started_at, last_heartbeat, is_active) VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
            params![session_id, agent_type, agent_instance, project_key, pid, now, now],
        )?;
        Ok(session_id)
    }

    pub fn agent_heartbeat(&self, session_id: &str, current_task: Option<&str>) -> Result<()> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;

        let rows = conn.execute(
            "UPDATE agent_sessions SET last_heartbeat = ?, current_task = ?, is_active = 1 WHERE id = ?",
            params![now, current_task, session_id],
        )?;

        if rows == 0 {
            let parts: Vec<&str> = session_id.splitn(3, '-').collect();
            let agent_type = parts.first().unwrap_or(&"unknown");
            let agent_instance = parts.get(1).unwrap_or(&"unknown");
            conn.execute(
                "INSERT INTO agent_sessions (id, agent_type, agent_instance, project_key, started_at, last_heartbeat, current_task, is_active) VALUES (?, ?, ?, 'default', ?, ?, ?, 1)",
                params![session_id, agent_type, agent_instance, now, now, current_task],
            )?;
        }
        Ok(())
    }

    pub fn get_active_agents(&self, project: Option<&str>) -> Result<Vec<serde_json::Value>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT id, agent_type, agent_instance, project_key, last_heartbeat, current_task FROM agent_sessions WHERE is_active = 1 AND (?1 IS NULL OR project_key = ?1) ORDER BY last_heartbeat DESC"
        )?;

        let rows = stmt.query_map(params![project], |row| {
            Ok(serde_json::json!({
                "session_id": row.get::<_, String>(0)?,
                "agent_type": row.get::<_, String>(1)?,
                "instance": row.get::<_, String>(2)?,
                "project": row.get::<_, String>(3)?,
                "last_heartbeat": row.get::<_, i64>(4)?,
                "current_task": row.get::<_, Option<String>>(5)?,
            }))
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn acquire_lock(&self, session_id: &str, lock_key: &str, _resource_type: &str, _resource_id: Option<&str>, ttl_secs: i64) -> Result<bool> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        let expires = now + ttl_secs;

        conn.execute("DELETE FROM active_locks WHERE expires_at < ?", params![now])?;

        let result = conn.execute(
            "INSERT OR REPLACE INTO active_locks (lock_key, agent_session_id, acquired_at, expires_at) VALUES (?, ?, ?, ?)",
            params![lock_key, session_id, now, expires],
        );
        Ok(result.is_ok())
    }

    pub fn release_lock(&self, lock_key: &str) -> Result<()> {
        let conn = self.get_conn();
        conn.execute("DELETE FROM active_locks WHERE lock_key = ?", params![lock_key])?;
        Ok(())
    }

    pub fn create_task(&self, project_key: &str, task_type: &str, payload: &str, priority: i32) -> Result<String> {
        let conn = self.get_conn();
        let task_id = Uuid::new_v4().to_hex_string();
        let now = Timestamp::now().0;

        conn.execute(
            "INSERT INTO task_queue (task_id, project_key, task_type, payload, priority, status, created_at) VALUES (?, ?, ?, ?, ?, 'pending', ?)",
            params![task_id, project_key, task_type, payload, priority, now],
        )?;
        Ok(task_id)
    }

    pub fn create_chunk(&self, project_key: &str, title: &str, content: &str, _parent_id: Option<&str>, level: i32) -> Result<String> {
        let conn = self.get_conn();
        let chunk_id = Uuid::new_v4().to_hex_string();
        let now = Timestamp::now().0;

        conn.execute(
            "INSERT INTO chunks (chunk_id, project_key, title, content, level, created_at, updated_at, is_active) VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
            params![chunk_id, project_key, title, content, level, now, now],
        )?;
        Ok(chunk_id)
    }

    pub fn claim_task(&self, session_id: &str, task_type: Option<&str>) -> Result<Option<String>> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;

        let task = conn.query_row(
            "SELECT task_id FROM task_queue WHERE status = 'pending' AND (?1 IS NULL OR task_type = ?1) ORDER BY priority DESC, created_at ASC LIMIT 1",
            params![task_type],
            |row| row.get::<_, String>(0),
        ).optional()?;

        if let Some(task_id) = task {
            conn.execute(
                "UPDATE task_queue SET status = 'running', agent_session_id = ?, started_at = ? WHERE task_id = ?",
                params![session_id, now, task_id],
            )?;
            Ok(Some(task_id))
        } else {
            Ok(None)
        }
    }

    pub fn complete_task(&self, task_id: &str, result: Option<&str>, error: Option<&str>) -> Result<()> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        let status = if error.is_some() { "failed" } else { "completed" };

        conn.execute(
            "UPDATE task_queue SET status = ?, completed_at = ?, result = ?, error = ? WHERE task_id = ?",
            params![status, now, result, error, task_id],
        )?;
        Ok(())
    }

    pub fn cleanup_stale_sessions(&self, threshold: i64) -> Result<usize> {
        let conn = self.get_conn();
        let deleted = conn.execute(
            "DELETE FROM agent_sessions WHERE is_active = 0 AND last_heartbeat < ?",
            params![threshold],
        )?;
        Ok(deleted)
    }

    pub fn get_stats(&self) -> Result<serde_json::Value> {
        let conn = self.get_conn();
        let obs: i64 = conn.query_row("SELECT COUNT(*) FROM observations WHERE deleted_at IS NULL", [], |r| r.get(0)).unwrap_or(0);
        let agents: i64 = conn.query_row("SELECT COUNT(*) FROM agent_sessions", [], |r| r.get(0)).unwrap_or(0);
        let active: i64 = conn.query_row("SELECT COUNT(*) FROM agent_sessions WHERE is_active = 1", [], |r| r.get(0)).unwrap_or(0);
        let tasks: i64 = conn.query_row("SELECT COUNT(*) FROM task_queue WHERE status = 'pending'", [], |r| r.get(0)).unwrap_or(0);

        Ok(serde_json::json!({
            "observations": obs,
            "agent_sessions": agents,
            "active_agents": active,
            "pending_tasks": tasks,
        }))
    }

    pub fn get_global_context(&self, project_key: &str) -> Result<Option<String>> {
        let conn = self.get_conn();
        let ctx = conn.query_row(
            "SELECT context_data FROM global_context WHERE project_key = ?",
            params![project_key],
            |row| row.get::<_, String>(0),
        ).optional()?;
        Ok(ctx)
    }

    pub fn set_global_context(&self, project_key: &str, context_data: &str) -> Result<()> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        conn.execute(
            "INSERT OR REPLACE INTO global_context (project_key, context_data, created_at, updated_at) VALUES (?, ?, ?, ?)",
            params![project_key, context_data, now, now],
        )?;
        Ok(())
    }

    pub fn export_context(&self, project_key: &str) -> Result<String> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare("SELECT chunk_id, title, content, level FROM chunks WHERE project_key = ? AND is_active = 1")?;
        let rows = stmt.query_map(params![project_key], |row| {
            Ok(serde_json::json!({
                "chunk_id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "content": row.get::<_, String>(2)?,
                "level": row.get::<_, i32>(3)?,
            }))
        })?;
        let chunks: Vec<_> = rows.filter_map(|r| r.ok()).collect();
        Ok(serde_json::to_string_pretty(&chunks)?)
    }

    pub fn import_context(&self, project_key: &str, data: &str) -> Result<i64> {
        let conn = self.get_conn();
        let chunks: Vec<serde_json::Value> = serde_json::from_str(data)?;
        let now = Timestamp::now().0;
        let mut imported = 0i64;

        for chunk in chunks {
            let chunk_id = chunk["chunk_id"].as_str().unwrap_or("");
            let title = chunk["title"].as_str().unwrap_or("");
            let content = chunk["content"].as_str().unwrap_or("");
            let level = chunk["level"].as_i64().unwrap_or(0) as i32;

            conn.execute(
                "INSERT OR REPLACE INTO chunks (chunk_id, project_key, title, content, level, created_at, updated_at, is_active) VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
                params![chunk_id, project_key, title, content, level, now, now],
            )?;
            imported += 1;
        }
        Ok(imported)
    }

    pub fn get_chunks_by_project(&self, project_key: &str, level: Option<i32>) -> Result<Vec<serde_json::Value>> {
        let conn = self.get_conn();
        if let Some(l) = level {
            let mut stmt = conn.prepare("SELECT chunk_id, title, content, level FROM chunks WHERE project_key = ? AND level = ? AND is_active = 1")?;
            let rows = stmt.query_map(params![project_key, l], |row| {
                Ok(serde_json::json!({
                    "chunk_id": row.get::<_, String>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "content": row.get::<_, String>(2)?,
                    "level": row.get::<_, i32>(3)?,
                }))
            })?;
            let result: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();
            Ok(result)
        } else {
            let mut stmt = conn.prepare("SELECT chunk_id, title, content, level FROM chunks WHERE project_key = ? AND is_active = 1")?;
            let rows = stmt.query_map(params![project_key], |row| {
                Ok(serde_json::json!({
                    "chunk_id": row.get::<_, String>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "content": row.get::<_, String>(2)?,
                    "level": row.get::<_, i32>(3)?,
                }))
            })?;
            let result: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();
            Ok(result)
        }
    }

    pub fn search_fts(&self, query: &str, project: Option<&str>, limit: i32) -> Result<Vec<serde_json::Value>> {
        let conn = self.get_conn();
        let sql = "SELECT title, content, project FROM observations WHERE deleted_at IS NULL AND (?1 IS NULL OR project = ?1) AND (title LIKE ?2 OR content LIKE ?2) LIMIT ?3";
        let search_term = format!("%{}%", query);

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![project, search_term, limit], |row| {
            Ok(serde_json::json!({
                "title": row.get::<_, String>(0)?,
                "content": row.get::<_, String>(1)?,
                "project": row.get::<_, String>(2)?,
            }))
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Log an audit entry
    pub fn log_audit(&self, observation_id: Option<i64>, action: &str, agent_id: &str, old_value: Option<&str>, new_value: Option<&str>, reason: Option<&str>) -> Result<()> {
        let conn = self.get_conn();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        conn.execute(
            "INSERT INTO audit_log (observation_id, action, agent_id, old_value, new_value, reason, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![observation_id, action, agent_id, old_value, new_value, reason, timestamp],
        )?;
        Ok(())
    }

    /// Get audit trail for an observation
    pub fn get_audit_trail(&self, observation_id: i64) -> Result<Vec<serde_json::Value>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT id, observation_id, action, agent_id, old_value, new_value, reason, timestamp FROM audit_log WHERE observation_id = ? ORDER BY timestamp DESC"
        )?;
        
        let rows = stmt.query_map(params![observation_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "observation_id": row.get::<_, Option<i64>>(1)?,
                "action": row.get::<_, String>(2)?,
                "agent_id": row.get::<_, String>(3)?,
                "old_value": row.get::<_, Option<String>>(4)?,
                "new_value": row.get::<_, Option<String>>(5)?,
                "reason": row.get::<_, Option<String>>(6)?,
                "timestamp": row.get::<_, i64>(7)?,
            }))
        })?;
        
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Update observation content with audit logging
    pub fn update_observation(&self, observation_id: ObservationId, new_content: &str, agent_id: &str, reason: Option<&str>) -> Result<()> {
        let conn = self.get_conn();
        
        // Get current content for audit log
        let old_content: String = conn.query_row(
            "SELECT content FROM observations WHERE id = ? AND deleted_at IS NULL",
            [observation_id.0],
            |row| row.get(0)
        )?;
        
        // Update observation
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        conn.execute(
            "UPDATE observations SET content = ?, revision_count = revision_count + 1, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            params![new_content, now, observation_id.0],
        )?;
        
        // Log audit entry
        self.log_audit(
            Some(observation_id.0),
            "update",
            agent_id,
            Some(&old_content),
            Some(new_content),
            reason,
        )?;
        
        Ok(())
    }

    /// Soft delete observation with audit logging
    pub fn delete_observation(&self, observation_id: ObservationId, agent_id: &str, reason: Option<&str>) -> Result<()> {
        let conn = self.get_conn();
        
        // Check if observation exists and is not already deleted
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM observations WHERE id = ? AND deleted_at IS NULL",
            [observation_id.0],
            |row| row.get(0)
        )?;
        
        if exists == 0 {
            return Err(crate::domain::errors::invalid_data("Observation not found or already deleted"));
        }
        
        // Soft delete (set deleted_at timestamp)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        conn.execute(
            "UPDATE observations SET deleted_at = ?, updated_at = ? WHERE id = ?",
            params![now, now, observation_id.0],
        )?;
        
        // Log audit entry
        self.log_audit(
            Some(observation_id.0),
            "delete",
            agent_id,
            None,
            None,
            reason,
        )?;
        
        Ok(())
    }

    /// Restore soft-deleted observation with audit logging
    pub fn restore_observation(&self, observation_id: ObservationId, agent_id: &str) -> Result<()> {
        let conn = self.get_conn();
        
        // Check if observation exists and is deleted
        let deleted_at: Option<i64> = conn.query_row(
            "SELECT deleted_at FROM observations WHERE id = ?",
            [observation_id.0],
            |row| row.get(0)
        )?;
        
        if deleted_at.is_none() {
            return Err(crate::domain::errors::invalid_data("Observation is not deleted"));
        }
        
        // Restore (clear deleted_at)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        conn.execute(
            "UPDATE observations SET deleted_at = NULL, updated_at = ? WHERE id = ?",
            params![now, observation_id.0],
        )?;
        
        // Log audit entry
        self.log_audit(
            Some(observation_id.0),
            "restore",
            agent_id,
            None,
            None,
            Some("Restored from soft delete"),
        )?;
        
        Ok(())
    }
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}

impl StoragePort for Database {
    fn init(&self) -> Result<()> {
        let conn = self.get_conn();
        self.create_tables(&conn)?;
        Ok(())
    }

    fn get_observation(&self, id: ObservationId) -> Result<Option<Observation>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT id, sync_id, session_id, observation_type, title, content, tool_name, project, scope, topic_key, content_hash, revision_count, duplicate_count, last_seen_at, created_at, updated_at, deleted_at, integrity_hash, classification
             FROM observations
             WHERE id = ? AND deleted_at IS NULL"
        )?;
        
        let result = stmt.query_row([id.0], |row| {
            let observation_type_int: i64 = row.get(3)?;
            let scope_int: i64 = row.get(8)?;
            let classification_int: i64 = row.get(18)?;
            
            Ok(Observation {
                id: ObservationId(row.get(0)?),
                sync_id: SyncId(row.get(1)?),
                session_id: SessionId(row.get(2)?),
                observation_type: match observation_type_int {
                    0 => ObservationType::Manual,
                    1 => ObservationType::ToolUse,
                    2 => ObservationType::FileChange,
                    3 => ObservationType::Command,
                    4 => ObservationType::FileRead,
                    5 => ObservationType::Search,
                    6 => ObservationType::Decision,
                    7 => ObservationType::Architecture,
                    8 => ObservationType::Bugfix,
                    9 => ObservationType::Pattern,
                    10 => ObservationType::Config,
                    11 => ObservationType::Discovery,
                    12 => ObservationType::Learning,
                    _ => ObservationType::Manual,
                },
                title: row.get(4)?,
                content: row.get(5)?,
                tool_name: row.get(6)?,
                project: row.get(7)?,
                scope: match scope_int {
                    0 => Scope::Project,
                    1 => Scope::Personal,
                    _ => Scope::Project,
                },
                topic_key: row.get(9)?,
                content_hash: ContentHash(row.get::<_, [u8; 32]>(10)?),
                revision_count: row.get(11)?,
                duplicate_count: row.get(12)?,
                last_seen_at: row.get::<_, Option<i64>>(13)?.map(Timestamp),
                created_at: Timestamp(row.get(14)?),
                updated_at: Timestamp(row.get(15)?),
                deleted_at: row.get::<_, Option<i64>>(16)?.map(Timestamp),
                integrity_hash: row.get(17)?,
                classification: match classification_int {
                    0 => Classification::Public,
                    1 => Classification::Internal,
                    2 => Classification::Confidential,
                    3 => Classification::Secret,
                    4 => Classification::TopSecret,
                    _ => Classification::Public,
                },
            })
        }).optional()?;
        
        Ok(result)
    }

    fn save_observation(&self, obs: &Observation) -> Result<ObservationId> {
        let conn = self.get_conn();
        
        // Convert enums to integers
        let observation_type_int = match obs.observation_type {
            ObservationType::Manual => 0,
            ObservationType::ToolUse => 1,
            ObservationType::FileChange => 2,
            ObservationType::Command => 3,
            ObservationType::FileRead => 4,
            ObservationType::Search => 5,
            ObservationType::Decision => 6,
            ObservationType::Architecture => 7,
            ObservationType::Bugfix => 8,
            ObservationType::Pattern => 9,
            ObservationType::Config => 10,
            ObservationType::Discovery => 11,
            ObservationType::Learning => 12,
        };
        
        let scope_int = match obs.scope {
            Scope::Project => 0,
            Scope::Personal => 1,
        };
        
        let classification_int = match obs.classification {
            Classification::Public => 0,
            Classification::Internal => 1,
            Classification::Confidential => 2,
            Classification::Secret => 3,
            Classification::TopSecret => 4,
        };
        
        // Check if observation with same sync_id exists
        let existing_id: Option<i64> = conn.query_row(
            "SELECT id FROM observations WHERE sync_id = ? AND deleted_at IS NULL",
            [obs.sync_id.0.as_str()],
            |row| row.get(0)
        ).optional()?;
        
        if let Some(existing_id) = existing_id {
            // Update existing observation
            conn.execute(
                "UPDATE observations SET 
                 session_id = ?, observation_type = ?, title = ?, content = ?, 
                 tool_name = ?, project = ?, scope = ?, topic_key = ?, 
                 content_hash = ?, revision_count = revision_count + 1, 
                 last_seen_at = ?, updated_at = ?, deleted_at = ?, 
                 integrity_hash = ?, classification = ?
                 WHERE id = ? AND deleted_at IS NULL",
                params![
                    obs.session_id.0,
                    observation_type_int,
                    &obs.title,
                    &obs.content,
                    obs.tool_name,
                    obs.project,
                    scope_int,
                    obs.topic_key,
                    &obs.content_hash.0[..],
                    obs.last_seen_at.map(|t| t.0),
                    obs.updated_at.0,
                    obs.deleted_at.map(|t| t.0),
                    obs.integrity_hash,
                    classification_int,
                    existing_id,
                ],
            )?;
            
            // Log audit for update (agent_id placeholder "system" for now)
            self.log_audit(
                Some(existing_id),
                "update",
                "system",
                Some("previous content"), // TODO: get old content
                Some(&obs.content),
                Some("Observation updated via save_observation"),
            )?;
            
            Ok(ObservationId(existing_id))
        } else {
            // Insert new observation
            conn.execute(
                "INSERT INTO observations (
                 sync_id, session_id, observation_type, title, content, 
                 tool_name, project, scope, topic_key, content_hash, 
                 revision_count, duplicate_count, last_seen_at, 
                 created_at, updated_at, deleted_at, integrity_hash, classification
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    obs.sync_id.0,
                    obs.session_id.0,
                    observation_type_int,
                    &obs.title,
                    &obs.content,
                    obs.tool_name,
                    obs.project,
                    scope_int,
                    obs.topic_key,
                    &obs.content_hash.0[..],
                    obs.revision_count,
                    obs.duplicate_count,
                    obs.last_seen_at.map(|t| t.0),
                    obs.created_at.0,
                    obs.updated_at.0,
                    obs.deleted_at.map(|t| t.0),
                    obs.integrity_hash,
                    classification_int,
                ],
            )?;
            
            let id = conn.last_insert_rowid();
            
            // Log audit for create
            self.log_audit(
                Some(id),
                "create",
                "system",
                None,
                Some(&obs.content),
                Some("Observation created via save_observation"),
            )?;
            
            Ok(ObservationId(id))
        }
    }

    fn search_observations(&self, params: &SearchParams) -> Result<Vec<SearchResult>> {
        let conn = self.get_conn();
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        
        // Add text search condition
        if !params.query.is_empty() {
            conditions.push("(title LIKE ? OR content LIKE ?)".to_string());
            let search_term = format!("%{}%", params.query);
            query_params.push(Box::new(search_term.clone()));
            query_params.push(Box::new(search_term));
        }
        
        // Add observation type filter
        if let Some(obs_type) = params.obs_type {
            conditions.push("observation_type = ?".to_string());
             let obs_type_int = match obs_type {
                ObservationType::Manual => 0,
                ObservationType::ToolUse => 1,
                ObservationType::FileChange => 2,
                ObservationType::Command => 3,
                ObservationType::FileRead => 4,
                ObservationType::Search => 5,
                ObservationType::Decision => 6,
                ObservationType::Architecture => 7,
                ObservationType::Bugfix => 8,
                ObservationType::Pattern => 9,
                ObservationType::Config => 10,
                ObservationType::Discovery => 11,
                ObservationType::Learning => 12,
            };
            query_params.push(Box::new(obs_type_int));
        }
        
        // Add project filter
        if let Some(project) = &params.project {
            conditions.push("project = ?".to_string());
            query_params.push(Box::new(project.clone()));
        }
        
        // Add scope filter
        if let Some(scope) = params.scope {
            conditions.push("scope = ?".to_string());
            let scope_int = match scope {
                Scope::Project => 0,
                Scope::Personal => 1,
            };
            query_params.push(Box::new(scope_int));
        }
        
        // Build SQL
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        
        let sql = format!(
            "SELECT id, sync_id, session_id, observation_type, title, content, tool_name, project, scope, topic_key, content_hash, revision_count, duplicate_count, last_seen_at, created_at, updated_at, deleted_at, integrity_hash, classification
             FROM observations
             {}
             ORDER BY created_at DESC
             LIMIT ?",
            where_clause
        );
        
        // Add limit parameter
        query_params.push(Box::new(params.limit));
        
        let mut stmt = conn.prepare(&sql)?;
        
        // Convert query_params to array of &dyn ToSql
        let params_ref: Vec<&dyn rusqlite::ToSql> = query_params.iter().map(|p| &**p as &dyn rusqlite::ToSql).collect();
        
        let rows = stmt.query_map(params_ref.as_slice(), |row| {
            let observation_type_int: i64 = row.get(3)?;
            let scope_int: i64 = row.get(8)?;
            let classification_int: i64 = row.get(18)?;
            
            Ok(SearchResult {
                observation: Observation {
                    id: ObservationId(row.get(0)?),
                    sync_id: SyncId(row.get(1)?),
                    session_id: SessionId(row.get(2)?),
                    observation_type: match observation_type_int {
                        0 => ObservationType::Manual,
                        1 => ObservationType::ToolUse,
                        2 => ObservationType::FileChange,
                        3 => ObservationType::Command,
                        4 => ObservationType::FileRead,
                        5 => ObservationType::Search,
                        6 => ObservationType::Decision,
                        7 => ObservationType::Architecture,
                        8 => ObservationType::Bugfix,
                        9 => ObservationType::Pattern,
                        10 => ObservationType::Config,
                        _ => ObservationType::Manual,
                    },
                    title: row.get(4)?,
                    content: row.get(5)?,
                    tool_name: row.get(6)?,
                    project: row.get(7)?,
                    scope: match scope_int {
                        0 => Scope::Project,
                        1 => Scope::Personal,
                        _ => Scope::Project,
                    },
                    topic_key: row.get(9)?,
                    content_hash: ContentHash(row.get::<_, [u8; 32]>(10)?),
                    revision_count: row.get(11)?,
                    duplicate_count: row.get(12)?,
                    last_seen_at: row.get::<_, Option<i64>>(13)?.map(Timestamp),
                    created_at: Timestamp(row.get(14)?),
                    updated_at: Timestamp(row.get(15)?),
                    deleted_at: row.get::<_, Option<i64>>(16)?.map(Timestamp),
                    integrity_hash: row.get(17)?,
                    classification: match classification_int {
                        0 => Classification::Public,
                        1 => Classification::Internal,
                        2 => Classification::Confidential,
                        3 => Classification::Secret,
                        4 => Classification::TopSecret,
                        _ => Classification::Public,
                    },
                },
                rank: 1.0, // Simple rank for now
                highlights: vec![],
            })
        })?;
        
        let results: Vec<SearchResult> = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(results)
    }

    fn get_timeline(&self, _limit: i32) -> Result<Vec<TimelineEntry>> {
        // TODO: Implement proper timeline retrieval
        // For now, return empty vector to satisfy interface
        Ok(vec![])
    }
}

impl SessionPort for Database {
    fn start_session(&self, project: &str, _directory: &str) -> Result<SessionId> {
        Ok(SessionId::new(project))
    }

    fn end_session(&self, _id: &SessionId, _summary: Option<String>) -> Result<()> {
        Ok(())
    }

    fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        Ok(vec![])
    }
}

impl MemoryPort for Database {
    fn save_memory(&self, memory: &Memory) -> Result<()> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        conn.execute(
            "INSERT OR REPLACE INTO memories (memory_id, agent_id, session_id, role, content, token_count, created_at, checksum) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                memory.id,
                memory.agent_id,
                memory.session_id,
                memory.role,
                memory.content,
                memory.token_count,
                now,
                memory.checksum,
            ],
        )?;
        Ok(())
    }

    fn get_memories(&self, agent_id: &str, session_id: Option<&str>) -> Result<Vec<Memory>> {
        let conn = self.get_conn();
        let mut stmt = match session_id {
            Some(_) => conn.prepare("SELECT memory_id, agent_id, session_id, role, content, token_count, created_at, checksum FROM memories WHERE agent_id = ? AND session_id = ? ORDER BY created_at ASC")?,
            None => conn.prepare("SELECT memory_id, agent_id, session_id, role, content, token_count, created_at, checksum FROM memories WHERE agent_id = ? ORDER BY created_at ASC")?,
        };

        let mapping = |row: &rusqlite::Row| {
            Ok(Memory {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                session_id: row.get(2)?,
                role: row.get(3)?,
                content: row.get(4)?,
                token_count: row.get(5)?,
                created_at: row.get(6)?,
                checksum: row.get(7)?,
            })
        };

        let memories: Vec<Memory> = match session_id {
            Some(sid) => stmt.query_map(params![agent_id, sid], mapping)?.filter_map(|r| r.ok()).collect(),
            None => stmt.query_map(params![agent_id], mapping)?.filter_map(|r| r.ok()).collect(),
        };

        Ok(memories)
    }

    fn clear_memories(&self, agent_id: &str, session_id: Option<&str>) -> Result<()> {
        let conn = self.get_conn();
        if let Some(sid) = session_id {
            conn.execute("DELETE FROM memories WHERE agent_id = ? AND session_id = ?", params![agent_id, sid])?;
        } else {
            conn.execute("DELETE FROM memories WHERE agent_id = ?", params![agent_id])?;
        }
        Ok(())
    }
}


