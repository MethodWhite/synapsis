//! Synapsis SQLite Database - Core Implementation

macro_rules! db_info {
    ($($arg:tt)*) => {{
        if !crate::config::is_quiet() {
            eprintln!($($arg)*);
        }
    }};
}

macro_rules! db_warn {
    ($($arg:tt)*) => {{
        eprintln!($($arg)*);
    }};
}

use crate::core::uuid::Uuid;
use crate::domain::ports::{SessionPort, StoragePort};
use crate::domain::*;
use base64::{Engine as _, engine::general_purpose};
use hex;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub mod multi_agent;

#[allow(dead_code)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    _data_dir: PathBuf,
    db_path: PathBuf,
    encryption_key: Option<Vec<u8>>,
}

impl Database {
    pub fn new() -> Self {
        let encryption_key = std::env::var("SYNAPSIS_DB_KEY")
            .ok()
            .and_then(|hex_key| hex::decode(hex_key).ok())
            .or_else(|| {
                std::env::var("SYNAPSIS_DB_KEY_BASE64")
                    .ok()
                    .and_then(|b64| general_purpose::STANDARD.decode(b64).ok())
            });
        Self::new_with_key(encryption_key)
    }

    pub fn new_with_key(encryption_key: Option<Vec<u8>>) -> Self {
        let data_dir = std::env::var("SYNAPSIS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_local_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("synapsis")
            });

        std::fs::create_dir_all(&data_dir).ok();
        let db_path = data_dir.join("synapsis.db");
        let conn = if let Some(key) = &encryption_key {
            match Connection::open(&db_path) {
                Ok(conn) => {
                    let hex_key = hex::encode(key);
                    let pragma = format!("PRAGMA key = x'{}'", hex_key);
                    if conn.execute_batch(&pragma).is_err() {
                        db_warn!("[Database] Warning: failed to set encryption key");
                    }
                    let _ = conn.execute_batch("PRAGMA cipher_version");
                    conn
                }
                Err(e) => {
                    db_warn!(
                        "[Database] Warning: failed to open encrypted DB ({}), trying unencrypted",
                        e
                    );
                    match Connection::open(&db_path) {
                        Ok(c) => c,
                        Err(e2) => {
                            db_warn!("[Database] Falling back to in-memory DB: {}", e2);
                            Connection::open_in_memory().expect("in-memory DB")
                        }
                    }
                }
            }
        } else {
            match Connection::open(&db_path) {
                Ok(c) => c,
                Err(e) => {
                    db_warn!("[Database] Falling back to in-memory DB: {}", e);
                    Connection::open_in_memory().expect("in-memory DB")
                }
            }
        };

        Self {
            conn: Arc::new(Mutex::new(conn)),
            _data_dir: data_dir.clone(),
            db_path,
            encryption_key,
        }
    }

    pub fn get_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn migrate_from_json(&self) -> Result<()> {
        // Look for JSON files in the data directory and import them
        let json_path = self._data_dir.join("observations.json");
        if json_path.exists() {
            db_info!("[Database] Migrating observations from JSON...");
            let content = match std::fs::read_to_string(&json_path) {
                Ok(c) => c,
                Err(e) => {
                    db_warn!("[Database] Cannot read {}: {}", json_path.display(), e);
                    return Ok(());
                }
            };
            let observations: Vec<serde_json::Value> = match serde_json::from_str(&content) {
                Ok(o) => o,
                Err(e) => {
                    db_warn!("[Database] Invalid JSON in {}: {}", json_path.display(), e);
                    return Ok(());
                }
            };
            let conn = self.get_conn();
            for obs_val in &observations {
                if let (Some(title), Some(content)) = (
                    obs_val.get("title").and_then(|t| t.as_str()),
                    obs_val.get("content").and_then(|c| c.as_str()),
                ) {
                    let session_id = obs_val
                        .get("session_id")
                        .and_then(|s| s.as_str())
                        .unwrap_or("migrated");
                    let now = Timestamp::now().0;
                    let sync_id = SyncId::new();
                    let obs_type: u8 = obs_val
                        .get("observation_type")
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0) as u8;
                    let scope: u8 =
                        obs_val.get("scope").and_then(|s| s.as_u64()).unwrap_or(0) as u8;
                    use sha2::Digest;
                    let hash = sha2::Sha256::digest(content.as_bytes());
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO observations (sync_id, session_id, project, observation_type, title, content, scope, content_hash, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![
                            sync_id.0,
                            session_id,
                            obs_val.get("project").and_then(|p| p.as_str()),
                            obs_type,
                            title,
                            content,
                            scope,
                            hash.as_slice(),
                            now,
                            now,
                        ],
                    ).ok();
                }
            }
            db_info!(
                "[Database] Imported {} observations from JSON",
                observations.len()
            );
        } else {
            db_info!(
                "[Database] No migration file found at {}",
                json_path.display()
            );
        }
        Ok(())
    }

    pub fn stats(&self) -> Result<serde_json::Value> {
        self.get_stats()
    }

    fn create_tables(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")
            .ok();
        let version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

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
            CREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(
                title, content
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
                action TEXT NOT NULL,
                observation_id INTEGER,
                agent_id TEXT,
                session_id TEXT,
                old_value TEXT,
                new_value TEXT,
                reason TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS memory_relations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sync_id TEXT NOT NULL UNIQUE,
                source_id INTEGER NOT NULL,
                target_id INTEGER NOT NULL,
                relation TEXT NOT NULL,
                judgment_status TEXT NOT NULL DEFAULT 'pending',
                reason TEXT,
                evidence TEXT,
                confidence REAL NOT NULL DEFAULT 1.0,
                marked_by_actor TEXT,
                marked_by_kind TEXT,
                marked_by_model TEXT,
                session_id TEXT,
                project TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (source_id) REFERENCES observations(id),
                FOREIGN KEY (target_id) REFERENCES observations(id)
            );
            ",
        )?;

        if (1..2).contains(&version) {
            let _ = conn.execute_batch(
                "INSERT INTO observations_fts(rowid, title, content) SELECT id, title, content FROM observations;
                 INSERT INTO schema_version (version) VALUES (2);"
            );
        }

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

    pub fn acquire_lock(
        &self,
        session_id: &str,
        lock_key: &str,
        _resource_type: &str,
        _resource_id: Option<&str>,
        ttl_secs: i64,
    ) -> Result<bool> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        let expires = now + ttl_secs;

        conn.execute(
            "DELETE FROM active_locks WHERE expires_at < ?",
            params![now],
        )?;

        // Only acquire if lock is free or expired
        let existing: std::result::Result<String, _> = conn.query_row(
            "SELECT agent_session_id FROM active_locks WHERE lock_key = ?1 AND expires_at > ?2",
            params![lock_key, now],
            |row| row.get(0),
        );
        match existing {
            Ok(owner) if owner != session_id => Ok(false),
            _ => {
                let result = conn.execute(
                        "INSERT OR REPLACE INTO active_locks (lock_key, agent_session_id, acquired_at, expires_at) VALUES (?, ?, ?, ?)",
                        params![lock_key, session_id, now, expires],
                    );
                Ok(result.is_ok())
            }
        }
    }

    pub fn release_lock(&self, lock_key: &str) -> Result<()> {
        let conn = self.get_conn();
        conn.execute(
            "DELETE FROM active_locks WHERE lock_key = ?",
            params![lock_key],
        )?;
        Ok(())
    }

    pub fn create_task(
        &self,
        project_key: &str,
        task_type: &str,
        payload: &str,
        priority: i32,
    ) -> Result<String> {
        let conn = self.get_conn();
        let task_id = Uuid::new_v4().to_hex_string();
        let now = Timestamp::now().0;

        conn.execute(
            "INSERT INTO task_queue (task_id, project_key, task_type, payload, priority, status, created_at) VALUES (?, ?, ?, ?, ?, 'pending', ?)",
            params![task_id, project_key, task_type, payload, priority, now],
        )?;
        Ok(task_id)
    }

    pub fn create_chunk(
        &self,
        project_key: &str,
        title: &str,
        content: &str,
        _parent_id: Option<&str>,
        level: i32,
    ) -> Result<String> {
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
            // Atomic claim: only succeed if still pending
            let updated = conn.execute(
                "UPDATE task_queue SET status = 'running', agent_session_id = ?, started_at = ? WHERE task_id = ? AND status = 'pending'",
                params![session_id, now, task_id],
            )?;
            if updated > 0 {
                Ok(Some(task_id))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn complete_task(
        &self,
        task_id: &str,
        result: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        let status = if error.is_some() {
            "failed"
        } else {
            "completed"
        };

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
        let obs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM observations WHERE deleted_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let agents: i64 = conn
            .query_row("SELECT COUNT(*) FROM agent_sessions", [], |r| r.get(0))
            .unwrap_or(0);
        let active: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_sessions WHERE is_active = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let tasks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_queue WHERE status = 'pending'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        Ok(serde_json::json!({
            "observations": obs,
            "agent_sessions": agents,
            "active_agents": active,
            "pending_tasks": tasks,
        }))
    }

    pub fn get_global_context(&self, project_key: &str) -> Result<Option<String>> {
        let conn = self.get_conn();
        let ctx = conn
            .query_row(
                "SELECT context_data FROM global_context WHERE project_key = ?",
                params![project_key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
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

    pub fn get_chunks_by_project(
        &self,
        project_key: &str,
        level: Option<i32>,
    ) -> Result<Vec<serde_json::Value>> {
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

    pub fn search_fts(
        &self,
        query: &str,
        project: Option<&str>,
        limit: i32,
    ) -> Result<Vec<serde_json::Value>> {
        let conn = self.get_conn();
        // Escape LIKE wildcards to prevent semantic injection
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let search_term = format!("%{}%", escaped);

        let mut stmt = if let Some(p) = project {
            let mut s = conn.prepare(
                "SELECT title, content, project FROM observations WHERE deleted_at IS NULL AND project = ?1 AND (title LIKE ?2 OR content LIKE ?2) ORDER BY created_at DESC LIMIT ?3"
            )?;
            let rows = s.query_map(rusqlite::params![p, search_term, limit], |row| {
                Ok(serde_json::json!({
                    "title": row.get::<_, String>(0)?,
                    "content": row.get::<_, String>(1)?,
                    "project": row.get::<_, Option<String>>(2)?,
                }))
            })?;
            return Ok(rows.filter_map(|r| r.ok()).collect());
        } else {
            conn.prepare(
                "SELECT title, content, project FROM observations WHERE deleted_at IS NULL AND (title LIKE ?1 OR content LIKE ?1) ORDER BY created_at DESC LIMIT ?2"
            )?
        };

        let rows = stmt.query_map(rusqlite::params![search_term, limit], |row| {
            Ok(serde_json::json!({
                "title": row.get::<_, String>(0)?,
                "content": row.get::<_, String>(1)?,
                "project": row.get::<_, Option<String>>(2)?,
            }))
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}

fn row_to_observation(row: &rusqlite::Row) -> rusqlite::Result<Observation> {
    Ok(Observation {
        id: ObservationId::new(row.get(0)?),
        sync_id: SyncId(row.get::<_, String>(1)?),
        session_id: SessionId::new(row.get::<_, String>(2)?),
        project: row.get(3)?,
        observation_type: {
            let v: u8 = row.get(4)?;
            match v {
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
            }
        },
        title: row.get(5)?,
        content: row.get(6)?,
        tool_name: row.get(7)?,
        scope: {
            let v: u8 = row.get(8)?;
            if v == 1 {
                Scope::Personal
            } else {
                Scope::Project
            }
        },
        topic_key: row.get(9)?,
        content_hash: {
            let hash_bytes: Vec<u8> = row.get::<_, Vec<u8>>(10).unwrap_or_default();
            let mut arr = [0u8; 32];
            let len = hash_bytes.len().min(32);
            arr[..len].copy_from_slice(&hash_bytes[..len]);
            ContentHash(arr)
        },
        revision_count: row.get(11)?,
        duplicate_count: row.get(12)?,
        last_seen_at: row.get::<_, Option<i64>>(13)?.map(Timestamp),
        created_at: Timestamp(row.get(14)?),
        updated_at: Timestamp(row.get(15)?),
        deleted_at: row.get::<_, Option<i64>>(16)?.map(Timestamp),
        integrity_hash: row.get(17)?,
        classification: {
            let v: u8 = row.get(18)?;
            match v {
                1 => Classification::Internal,
                2 => Classification::Confidential,
                3 => Classification::Secret,
                4 => Classification::TopSecret,
                _ => Classification::Public,
            }
        },
    })
}

impl Database {
    pub fn soft_delete_observation(&self, id: i64) -> Result<()> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        conn.execute(
            "UPDATE observations SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            rusqlite::params![now, id],
        )?;
        let _ = conn.execute(
            "INSERT INTO observations_fts(observations_fts, rowid, title, content) VALUES('delete', ?1, '', '')",
            params![id],
        );
        Ok(())
    }

    pub fn get_timeline_direct(&self, limit: i32) -> Result<Vec<TimelineEntry>> {
        self.get_timeline(limit)
    }

    pub fn save_observation_direct(&self, obs: &Observation) -> Result<ObservationId> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        conn.execute(
            "INSERT INTO observations (sync_id, session_id, project, observation_type, title, content, tool_name, scope, topic_key, content_hash, revision_count, duplicate_count, created_at, updated_at, classification)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                obs.sync_id.0,
                obs.session_id.0,
                obs.project,
                obs.observation_type as u8,
                obs.title,
                obs.content,
                obs.tool_name,
                obs.scope as u8,
                obs.topic_key,
                obs.content_hash.as_bytes(),
                obs.revision_count,
                obs.duplicate_count,
                now,
                now,
                obs.classification as u8,
            ],
        )?;
        let id = conn.last_insert_rowid();
        let _ = conn.execute(
            "INSERT INTO observations_fts(rowid, title, content) VALUES (?1, ?2, ?3)",
            params![id, obs.title, obs.content],
        );
        let _ =
            conn.execute_batch("INSERT INTO observations_fts(observations_fts) VALUES('rebuild')");
        Ok(ObservationId::new(id))
    }

    fn sanitize_fts_query(query: &str) -> String {
        query
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '_')
            .collect::<String>()
            .split_whitespace()
            .filter(|w| {
                !matches!(
                    *w,
                    "or" | "and" | "not" | "near" | "NEAR" | "OR" | "AND" | "NOT"
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn search_fts5(
        &self,
        query: &str,
        project: Option<&str>,
        scope: Option<&str>,
        limit: i32,
    ) -> Result<Vec<serde_json::Value>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT o.id, o.title, o.content, o.project, o.created_at, o.observation_type, o.scope
             FROM observations_fts
             JOIN observations o ON o.id = observations_fts.rowid
             WHERE observations_fts MATCH ?1
             AND o.deleted_at IS NULL
             AND (?2 IS NULL OR o.project = ?2)
             AND (?3 IS NULL OR o.scope = ?3)
             ORDER BY rank
             LIMIT ?4",
        )?;
        let sanitized = Self::sanitize_fts_query(query);
        let scope_int = scope.map(|s| match s {
            "personal" => 1u8,
            _ => 0u8,
        });
        let rows = stmt.query_map(params![sanitized, project, scope_int, limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "title": row.get::<_, String>(1)?,
                "content": row.get::<_, String>(2)?,
                "project": row.get::<_, Option<String>>(3)?,
                "created_at": row.get::<_, i64>(4)?,
                "observation_type": row.get::<_, u8>(5)?,
                "scope": row.get::<_, u8>(6)?,
            }))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn update_observation(&self, id: i64, title: &str, content: &str) -> Result<()> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        conn.execute(
            "UPDATE observations SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4 AND deleted_at IS NULL",
            params![title, content, now, id],
        )?;
        let _ = conn.execute(
            "INSERT INTO observations_fts(observations_fts, rowid, title, content) VALUES('delete', ?1, '', '')",
            params![id],
        );
        let _ = conn.execute(
            "INSERT INTO observations_fts(rowid, title, content) VALUES (?1, ?2, ?3)",
            params![id, title, content],
        );
        Ok(())
    }

    pub fn insert_relation(
        &self,
        source_id: i64,
        target_id: i64,
        relation: &str,
        reason: Option<&str>,
        evidence: Option<&str>,
        confidence: f64,
        session_id: Option<&str>,
        project: Option<&str>,
    ) -> Result<String> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        let sync_id = format!("rel-{}", Uuid::new_v4().to_hex_string());
        conn.execute(
            "INSERT INTO memory_relations (sync_id, source_id, target_id, relation, judgment_status, reason, evidence, confidence, marked_by_actor, marked_by_kind, session_id, project, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'judged', ?5, ?6, ?7, 'engram', 'system', ?8, ?9, ?10, ?10)",
            params![sync_id, source_id, target_id, relation, reason, evidence, confidence, session_id, project, now],
        )?;
        Ok(sync_id)
    }

    pub fn get_relations_for_observation(&self, obs_id: i64) -> Result<Vec<serde_json::Value>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT r.id, r.sync_id, r.source_id, r.target_id, r.relation, r.judgment_status, r.reason, r.confidence, r.created_at,
                    o.title AS target_title
             FROM memory_relations r
             LEFT JOIN observations o ON o.id = r.target_id
             WHERE (r.source_id = ?1 OR r.target_id = ?1) AND r.judgment_status = 'judged'"
        )?;
        let rows = stmt.query_map(params![obs_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "sync_id": row.get::<_, String>(1)?,
                "source_id": row.get::<_, i64>(2)?,
                "target_id": row.get::<_, i64>(3)?,
                "relation": row.get::<_, String>(4)?,
                "judgment_status": row.get::<_, String>(5)?,
                "reason": row.get::<_, Option<String>>(6)?,
                "confidence": row.get::<_, f64>(7)?,
                "created_at": row.get::<_, i64>(8)?,
                "target_title": row.get::<_, Option<String>>(9)?.unwrap_or_default(),
            }))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_observation_by_id(&self, id: i64) -> Result<Option<serde_json::Value>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT id, title, content, project, observation_type, created_at, updated_at FROM observations WHERE id = ?1 AND deleted_at IS NULL"
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "title": row.get::<_, String>(1)?,
                "content": row.get::<_, String>(2)?,
                "project": row.get::<_, Option<String>>(3)?,
                "observation_type": row.get::<_, u8>(4)?,
                "created_at": row.get::<_, i64>(5)?,
                "updated_at": row.get::<_, i64>(6)?,
            }))
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn get_session_stats(&self, session_id: &str) -> Result<serde_json::Value> {
        let conn = self.get_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM observations WHERE session_id = ?1 AND deleted_at IS NULL",
                params![session_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let first: Option<i64> = conn.query_row(
            "SELECT MIN(created_at) FROM observations WHERE session_id = ?1 AND deleted_at IS NULL",
            params![session_id], |r| r.get(0),
        ).optional().ok().flatten();
        let last: Option<i64> = conn.query_row(
            "SELECT MAX(updated_at) FROM observations WHERE session_id = ?1 AND deleted_at IS NULL",
            params![session_id], |r| r.get(0),
        ).optional().ok().flatten();
        Ok(serde_json::json!({
            "session_id": session_id,
            "observation_count": count,
            "first_seen": first,
            "last_seen": last,
        }))
    }

    pub fn merge_projects(&self, source_project: &str, target_project: &str) -> Result<i64> {
        let conn = self.get_conn();
        let updated = conn.execute(
            "UPDATE observations SET project = ?1 WHERE project = ?2 AND deleted_at IS NULL",
            params![target_project, source_project],
        )?;
        Ok(updated as i64)
    }

    pub fn log_audit(
        &self,
        action: &str,
        observation_id: Option<i64>,
        agent_id: Option<&str>,
        session_id: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
        reason: Option<&str>,
    ) -> Result<()> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        conn.execute(
            "INSERT INTO audit_log (action, observation_id, agent_id, session_id, old_value, new_value, reason, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![action, observation_id, agent_id, session_id, old_value, new_value, reason, now],
        )?;
        Ok(())
    }

    pub fn get_audit_trail(&self, limit: i32) -> Result<Vec<serde_json::Value>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT id, action, observation_id, agent_id, session_id, reason, created_at FROM audit_log ORDER BY created_at DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "action": row.get::<_, String>(1)?,
                "observation_id": row.get::<_, Option<i64>>(2)?,
                "agent_id": row.get::<_, Option<String>>(3)?,
                "session_id": row.get::<_, Option<String>>(4)?,
                "reason": row.get::<_, Option<String>>(5)?,
                "created_at": row.get::<_, i64>(6)?,
            }))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn doctor_check(&self) -> Result<serde_json::Value> {
        let conn = self.get_conn();
        let obs_ok = conn
            .query_row("SELECT COUNT(*) FROM observations", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap_or(-1);
        let deleted = conn
            .query_row(
                "SELECT COUNT(*) FROM observations WHERE deleted_at IS NOT NULL",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(-1);
        let sessions_ok = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get::<_, i64>(0))
            .unwrap_or(-1);
        let fts_ok = conn
            .query_row("SELECT COUNT(*) FROM observations_fts", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap_or(-1);
        let integrity = conn
            .query_row(
                "SELECT COUNT(*) FROM observations WHERE content_hash = X'00'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(-1);
        let broken_relations = conn.query_row(
            "SELECT COUNT(*) FROM memory_relations r LEFT JOIN observations o ON o.id = r.source_id WHERE o.id IS NULL",
            [], |r| r.get::<_, i64>(0),
        ).unwrap_or(-1);
        Ok(serde_json::json!({
            "status": if obs_ok >= 0 { "ok" } else { "error" },
            "observations": obs_ok.max(0),
            "deleted": deleted.max(0),
            "sessions": sessions_ok.max(0),
            "fts_entries": fts_ok.max(0),
            "zero_hash_observations": integrity.max(0),
            "broken_relations": broken_relations.max(0),
        }))
    }

    pub fn get_observation_by_id_raw(
        &self,
        id: i64,
    ) -> Result<Option<(String, String, String, Option<String>)>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT title, content, observation_type, project FROM observations WHERE id = ?1 AND deleted_at IS NULL"
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn backup_to(&self, path: &std::path::Path) -> Result<()> {
        let path_str = path.display().to_string();
        if !path_str.chars().all(|c| {
            c.is_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.' || c == ':'
        }) {
            return Err(SynapsisError::internal_bug(
                "Backup path contains invalid characters".to_string(),
            ));
        }
        let conn = self.get_conn();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SynapsisError::internal_bug(e.to_string()))?;
        }
        conn.execute_batch(&format!("VACUUM INTO '{}'", path_str.replace('\'', "''")))
            .map_err(|e| SynapsisError::internal_bug(e.to_string()))?;
        db_info!("[Database] Backup saved to {}", path.display());
        Ok(())
    }

    pub fn integrity_check(&self) -> Result<Vec<String>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare("PRAGMA integrity_check")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let results: Vec<String> = rows.filter_map(|r| r.ok()).collect();
        if results.iter().any(|r| r != "ok") {
            db_warn!("[Database] Integrity check found issues: {:?}", results);
        }
        Ok(results)
    }

    pub fn quick_check(&self) -> Result<bool> {
        let results = self.integrity_check()?;
        Ok(results.iter().all(|r| r == "ok"))
    }

    pub fn prune_observations(&self, older_than_days: i64) -> Result<u64> {
        let conn = self.get_conn();
        let cutoff = Timestamp::now().0 - (older_than_days * 86400);
        let count = conn.execute(
            "UPDATE observations SET deleted_at = ?1 WHERE deleted_at IS NULL AND created_at < ?1",
            params![cutoff],
        )?;
        if count > 0 {
            db_info!(
                "[Database] Pruned {} observations older than {} days",
                count,
                older_than_days
            );
        }
        Ok(count as u64)
    }

    pub fn vacuum(&self) -> Result<()> {
        let conn = self.get_conn();
        conn.execute_batch("VACUUM")
            .map_err(|e| SynapsisError::internal_bug(e.to_string()))?;
        db_info!("[Database] VACUUM completed");
        Ok(())
    }
}

impl StoragePort for Database {
    fn init(&self) -> Result<()> {
        let conn = self.get_conn();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS _schema_version (version INTEGER NOT NULL)")
            .ok();
        let version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if version < 1 {
            conn.execute("INSERT INTO _schema_version (version) VALUES (1)", [])
                .ok();
        }
        self.create_tables(&conn)?;
        Ok(())
    }

    fn get_observation(&self, id: ObservationId) -> Result<Option<Observation>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT id, sync_id, session_id, project, observation_type, title, content, tool_name, scope, topic_key, content_hash, revision_count, duplicate_count, last_seen_at, created_at, updated_at, deleted_at, integrity_hash, classification
             FROM observations WHERE id = ?1 AND deleted_at IS NULL"
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id.0], row_to_observation)?;
        Ok(rows.next().transpose()?)
    }

    fn save_observation(&self, obs: &Observation) -> Result<ObservationId> {
        self.save_observation_direct(obs)
    }

    fn search_observations(&self, params: &SearchParams) -> Result<Vec<SearchResult>> {
        let conn = self.get_conn();
        let escaped = params.query.replace("%", r"\%").replace("_", r"\_");
        let search_term = format!("%{}%", escaped);
        let mut stmt = conn.prepare(
            "SELECT id, sync_id, session_id, project, observation_type, title, content, tool_name, scope, topic_key, content_hash, revision_count, duplicate_count, last_seen_at, created_at, updated_at, deleted_at, integrity_hash, classification
             FROM observations WHERE deleted_at IS NULL AND (title LIKE ?1 OR content LIKE ?1) ORDER BY created_at DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(
            rusqlite::params![search_term, params.limit],
            row_to_observation,
        )?;
        let observations: Vec<Observation> = rows.filter_map(|r| r.ok()).collect();
        Ok(observations
            .into_iter()
            .map(|o| SearchResult::new(o, 0.0))
            .collect())
    }

    fn get_timeline(&self, limit: i32) -> Result<Vec<TimelineEntry>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT id, sync_id, session_id, project, observation_type, title, content, tool_name, scope, topic_key, content_hash, revision_count, duplicate_count, last_seen_at, created_at, updated_at, deleted_at, integrity_hash, classification
             FROM observations WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(rusqlite::params![limit], row_to_observation)?;
        let observations: Vec<Observation> = rows.filter_map(|r| r.ok()).collect();
        Ok(observations
            .into_iter()
            .map(|o| TimelineEntry {
                observation: o,
                is_focus: false,
            })
            .collect())
    }
}

impl SessionPort for Database {
    fn start_session(&self, project: &str, directory: &str) -> Result<SessionId> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        let id = SessionId::new(directory);
        conn.execute(
            "INSERT OR IGNORE INTO sessions (id, project_key, directory, started_at, observation_count) VALUES (?1, ?2, ?3, ?4, 0)",
            params![id.0, project, directory, now],
        )?;
        Ok(id)
    }

    fn end_session(&self, id: &SessionId, summary: Option<String>) -> Result<()> {
        let conn = self.get_conn();
        let now = Timestamp::now().0;
        conn.execute(
            "UPDATE sessions SET ended_at = ?1, summary = COALESCE(?2, summary) WHERE id = ?3",
            params![now, summary, id.0],
        )?;
        Ok(())
    }

    fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let conn = self.get_conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_key, started_at, ended_at, summary, observation_count FROM sessions ORDER BY started_at DESC LIMIT 50"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SessionSummary {
                id: SessionId::new(row.get::<_, String>(0)?),
                project: row.get(1)?,
                started_at: Timestamp(row.get(2)?),
                ended_at: row.get::<_, Option<i64>>(3)?.map(Timestamp),
                summary: row.get(4)?,
                observation_count: row.get(5)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
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
            Some(sid) => stmt
                .query_map(params![agent_id, sid], mapping)?
                .filter_map(|r| r.ok())
                .collect(),
            None => stmt
                .query_map(params![agent_id], mapping)?
                .filter_map(|r| r.ok())
                .collect(),
        };

        Ok(memories)
    }

    fn clear_memories(&self, agent_id: &str, session_id: Option<&str>) -> Result<()> {
        let conn = self.get_conn();
        if let Some(sid) = session_id {
            conn.execute(
                "DELETE FROM memories WHERE agent_id = ? AND session_id = ?",
                params![agent_id, sid],
            )?;
        } else {
            conn.execute("DELETE FROM memories WHERE agent_id = ?", params![agent_id])?;
        }
        Ok(())
    }
}
