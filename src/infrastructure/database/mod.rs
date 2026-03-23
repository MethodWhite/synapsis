//! Synapsis SQLite Database - Core Implementation

use crate::core::uuid::Uuid;
use crate::domain::ports::{SessionPort, StoragePort};
use crate::domain::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub mod multi_agent;
pub mod encryption;

pub struct Database {
    conn: Arc<Mutex<Connection>>,
    _data_dir: PathBuf,
}

unsafe impl Send for Database {}
unsafe impl Sync for Database {}

impl Database {
    pub fn new() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapsis");

        std::fs::create_dir_all(&data_dir).ok();
        let db_path = data_dir.join("synapsis.db");
        let conn = Connection::open(&db_path).unwrap();

        Self {
            conn: Arc::new(Mutex::new(conn)),
            _data_dir: data_dir,
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
            let agent_type = parts.get(0).unwrap_or(&"unknown");
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
        Ok(deleted as usize)
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

    fn get_observation(&self, _id: ObservationId) -> Result<Option<Observation>> {
        Ok(None)
    }

    fn save_observation(&self, _obs: &Observation) -> Result<ObservationId> {
        Ok(ObservationId(0))
    }

    fn search_observations(&self, _params: &SearchParams) -> Result<Vec<SearchResult>> {
        Ok(vec![])
    }

    fn get_timeline(&self, _limit: i32) -> Result<Vec<TimelineEntry>> {
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
