//! Database migration system for Synapsis
//! Each migration is a numbered step that can be applied sequentially.

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Migration {
    pub version: u32,
    pub name: String,
    pub description: String,
    pub applied_at: Option<i64>,
}

type MigrationFn = fn(&Connection) -> Result<()>;

fn migration_v1_initial_schema(conn: &Connection) -> Result<()> {
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
            scope INTEGER NOT NULL DEFAULT 0,
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
        CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
    ",
    )?;
    Ok(())
}

fn migration_v2_fts_index(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(title, content);",
    )?;
    conn.execute_batch(
        "INSERT OR IGNORE INTO observations_fts(rowid, title, content) SELECT id, title, content FROM observations;",
    )?;
    Ok(())
}

fn migration_v3_add_agent_sessions(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
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
    ",
    )?;
    Ok(())
}

fn migration_v4_add_audit_log(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
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
    ",
    )?;
    Ok(())
}

fn migration_v5_add_memory_relations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
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
    ",
    )?;
    Ok(())
}

fn migration_v6_add_x402_payments(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS x402_payments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tx_hash TEXT NOT NULL UNIQUE,
            feature TEXT NOT NULL,
            amount_usdc REAL NOT NULL,
            payer_wallet TEXT NOT NULL,
            verified_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS licenses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            customer TEXT NOT NULL,
            license_type TEXT NOT NULL,
            features TEXT NOT NULL,
            issued_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            signature TEXT NOT NULL,
            UNIQUE(customer, license_type)
        );
    ",
    )?;
    Ok(())
}

fn migration_v7_add_audit_chain(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "ALTER TABLE audit_log ADD COLUMN prev_hash TEXT DEFAULT '0000000000000000000000000000000000000000000000000000000000000000';
         ALTER TABLE audit_log ADD COLUMN data_hash TEXT DEFAULT '';
         ALTER TABLE audit_log ADD COLUMN chain_hash TEXT DEFAULT '';"
    )?;
    Ok(())
}

/// Backfill audit_log entries that were created before the audit chain columns
/// existed (v7 added them with empty defaults, leaving old rows un-hashed).
/// Recomputes data_hash and chain_hash in order so verify_audit_chain passes.
fn migration_v8_backfill_audit_chain(conn: &Connection) -> Result<()> {
    use sha2::{Digest, Sha256};

    let mut prev: String =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();

    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM audit_log ORDER BY id ASC")?
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    for id in ids {
        let row: Option<(String, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, i64)> =
            conn.query_row(
                "SELECT action, observation_id, agent_id, session_id, old_value, new_value, reason, created_at
                 FROM audit_log WHERE id = ?1",
                [id],
                |r| Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?,
                    r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?,
                )),
            )
            .optional()?;

        let Some((action, oid, agent, session, old_v, new_v, reason, ts)) = row else {
            continue;
        };

        let details = format!(
            "action={} oid={:?} agent={:?} session={:?} old={:?} new={:?} reason={:?}",
            action, oid, agent, session, old_v, new_v, reason
        );
        let data_hash = hex::encode(Sha256::digest(details.as_bytes()));
        let chain_hash = hex::encode(Sha256::digest(
            format!("{}:{}:{}", prev, data_hash, ts).as_bytes(),
        ));

        conn.execute(
            "UPDATE audit_log SET prev_hash = ?1, data_hash = ?2, chain_hash = ?3 WHERE id = ?4",
            rusqlite::params![prev, data_hash, chain_hash, id],
        )?;
        prev = chain_hash;
    }

    Ok(())
}

/// Sequential thinking trees: reasoning steps that branch and track depth.
/// Keeps reasoning state inside Synapsis instead of depending on an external
/// MCP server like sequential-thinking.
fn migration_v9_add_thinking(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS thinking_trees (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tree_id TEXT NOT NULL UNIQUE,
            project TEXT,
            session_id TEXT,
            topic TEXT,
            status TEXT NOT NULL DEFAULT 'active',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS thinking_steps (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tree_id TEXT NOT NULL,
            step_index INTEGER NOT NULL,
            parent_index INTEGER,
            branch INTEGER NOT NULL DEFAULT 0,
            thought TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE(tree_id, branch, step_index)
        );"
    )?;
    Ok(())
}

/// Cross-platform message mailbox: agents publish structured messages that
/// peers in the same project can consume, so IDE/TUI/CLI sessions communicate
/// intelligently instead of only receiving flat notification strings.
fn migration_v10_add_bridge_messages(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS bridge_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id TEXT NOT NULL UNIQUE,
            project TEXT NOT NULL,
            from_session TEXT NOT NULL,
            from_agent TEXT NOT NULL,
            to_session TEXT,
            message_type TEXT NOT NULL DEFAULT 'observation',
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            delivered INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_bridge_messages_project
            ON bridge_messages(project, delivered, created_at);"
    )?;
    Ok(())
}

/// Registry of all migrations. Add new migrations at the END.
pub fn all_migrations() -> Vec<MigrationFn> {
    vec![
        migration_v1_initial_schema,
        migration_v2_fts_index,
        migration_v3_add_agent_sessions,
        migration_v4_add_audit_log,
        migration_v5_add_memory_relations,
        migration_v6_add_x402_payments,
        migration_v7_add_audit_chain,
        migration_v8_backfill_audit_chain,
        migration_v9_add_thinking,
        migration_v10_add_bridge_messages,
    ]
}

const MIGRATION_NAMES: &[&str] = &[
    "v1_initial",
    "v2_fts",
    "v3_agent_sessions",
    "v4_audit_log",
    "v5_memory_relations",
    "v6_x402",
    "v7_audit_chain",
    "v8_backfill_audit_chain",
    "v9_thinking",
    "v10_bridge_messages",
];

/// Run all pending migrations. Returns (current_version, migrations_applied).
pub fn run_migrations(conn: &Connection) -> Result<(u32, u32)> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")
        .ok();

    let current: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let migrations = all_migrations();
    let mut applied = 0;

    for (i, migration) in migrations.iter().enumerate() {
        let version = (i + 1) as u32;
        if version > current {
            let name = MIGRATION_NAMES.get(i).unwrap_or(&"unknown");
            migration(conn).with_context(|| format!("Migration {} ({}) failed", version, name))?;
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                rusqlite::params![version],
            )?;
            applied += 1;
            eprintln!("[DB] Migration {}: {} applied", version, name);
        }
    }

    Ok((current, applied))
}

/// Get the current migration status as JSON.
pub fn get_migration_status(conn: &Connection) -> Result<serde_json::Value> {
    let current: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn.prepare("SELECT version FROM schema_version ORDER BY version ASC")?;
    let versions: Vec<u32> = stmt
        .query_map([], |r| r.get::<_, u32>(0))?
        .filter_map(|r| r.ok())
        .collect();

    let total = all_migrations().len() as u32;

    let applied: Vec<serde_json::Value> = versions
        .iter()
        .map(|v| {
            let name = MIGRATION_NAMES.get((*v - 1) as usize).unwrap_or(&"unknown");
            serde_json::json!({
                "version": v,
                "name": name,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "current_version": current,
        "total_migrations": total,
        "applied_migrations": applied,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_migrations_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        let (current, applied) = run_migrations(&conn).unwrap();
        assert_eq!(current, 0);
        assert_eq!(applied, 7);
        let status = get_migration_status(&conn).unwrap();
        assert_eq!(status["current_version"], 7);
    }

    #[test]
    fn test_run_migrations_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let (current, applied) = run_migrations(&conn).unwrap();
        assert!(current >= 6);
        assert_eq!(applied, 0);
    }
}
