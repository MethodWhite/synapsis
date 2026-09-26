//! Database migration system for Synapsis
//! Each migration is a numbered step that can be applied sequentially.

use anyhow::{Context, Result};
use rusqlite::{Connection, Transaction, TransactionBehavior};
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
/// Existing hashed entries must already form a valid chain; never normalize or
/// overwrite evidence of a disconnected or partially populated chain.
fn migration_v8_backfill_audit_chain(conn: &Connection) -> Result<()> {
    use sha2::{Digest, Sha256};

    type AuditRow = (
        i64,
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
        i64,
    );
    let rows: Vec<AuditRow> = conn
        .prepare(
            "SELECT id, action, observation_id, agent_id, session_id, old_value, new_value, reason,
                    prev_hash, data_hash, chain_hash, created_at
             FROM audit_log ORDER BY id ASC",
        )?
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
                r.get(11)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let zero_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let mut prev = zero_hash.to_string();
    let mut seen_hashed_row = false;
    for (
        id,
        action,
        oid,
        agent,
        session,
        old_v,
        new_v,
        reason,
        stored_prev,
        stored_data,
        stored_chain,
        ts,
    ) in rows
    {
        let details = format!(
            "action={} oid={:?} agent={:?} session={:?} old={:?} new={:?} reason={:?}",
            action, oid, agent, session, old_v, new_v, reason
        );
        let computed_data = hex::encode(Sha256::digest(details.as_bytes()));
        if !stored_data.is_empty() && stored_data != computed_data {
            return Err(anyhow::anyhow!(
                "Migration v8 refused to rewrite audit row {id}: stored data hash does not match its event"
            ));
        }
        let is_legacy_unhashed =
            stored_prev == zero_hash && stored_data.is_empty() && stored_chain.is_empty();
        let chain_hash = if is_legacy_unhashed {
            if seen_hashed_row {
                return Err(anyhow::anyhow!(
                    "Migration v8 refused to rewrite audit row {id}: an unhashed row follows hashed audit history"
                ));
            }
            let chain_hash = hex::encode(Sha256::digest(
                format!("{}:{}:{}", prev, computed_data, ts).as_bytes(),
            ));
            conn.execute(
                "UPDATE audit_log SET prev_hash = ?1, data_hash = ?2, chain_hash = ?3 WHERE id = ?4",
                rusqlite::params![prev, computed_data, chain_hash, id],
            )?;
            chain_hash
        } else {
            if stored_data.is_empty() || stored_chain.is_empty() {
                return Err(anyhow::anyhow!(
                    "Migration v8 refused to rewrite audit row {id}: hash fields are only partially populated"
                ));
            }
            if stored_prev != prev {
                return Err(anyhow::anyhow!(
                    "Migration v8 refused to rewrite audit row {id}: previous hash does not link to the preceding row"
                ));
            }
            if stored_data != computed_data {
                return Err(anyhow::anyhow!(
                    "Migration v8 refused to rewrite audit row {id}: stored data hash does not match its event"
                ));
            }
            let expected_chain = hex::encode(Sha256::digest(
                format!("{}:{}:{}", prev, stored_data, ts).as_bytes(),
            ));
            if stored_chain != expected_chain {
                return Err(anyhow::anyhow!(
                    "Migration v8 refused to rewrite audit row {id}: stored chain hash is inconsistent"
                ));
            }
            seen_hashed_row = true;
            stored_chain
        };

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
        );",
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
            ON bridge_messages(project, delivered, created_at);",
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
    // Serialize concurrent process startups before reading the version, and make
    // each migration set atomic. The timeout lets another starter finish first.
    conn.busy_timeout(std::time::Duration::from_secs(30))
        .context("configure migration lock wait")?;
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .context("begin immediate migration transaction")?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")?;

    let current: u32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |r| r.get(0),
    )?;

    let migrations = all_migrations();
    let mut applied = 0;

    for (i, migration) in migrations.iter().enumerate() {
        let version = (i + 1) as u32;
        if version > current {
            let name = MIGRATION_NAMES.get(i).unwrap_or(&"unknown");
            migration(&tx).with_context(|| format!("Migration {} ({}) failed", version, name))?;
            tx.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                rusqlite::params![version],
            )?;
            applied += 1;
            eprintln!("[DB] Migration {}: {} applied", version, name);
        }
    }

    let final_version = current.max(migrations.len() as u32);
    tx.commit().context("commit migration transaction")?;
    Ok((final_version, applied))
}

/// Get the current migration status as JSON.
pub fn get_migration_status(conn: &Connection) -> Result<serde_json::Value> {
    let current: u32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |r| r.get(0),
    )?;

    let mut stmt = conn.prepare("SELECT version FROM schema_version ORDER BY version ASC")?;
    let versions: Vec<u32> = stmt
        .query_map([], |r| r.get::<_, u32>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

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
    use sha2::{Digest, Sha256};

    fn version_7_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE schema_version (version INTEGER NOT NULL)")
            .unwrap();
        for (index, migration) in all_migrations().iter().take(7).enumerate() {
            migration(&conn).unwrap();
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [index as u32 + 1],
            )
            .unwrap();
        }
        conn
    }

    fn insert_valid_hashed_audit_row(conn: &Connection, action: &str, created_at: i64) {
        let oid = Some(42_i64);
        let agent = Some("test-agent".to_string());
        let session = Some("test-session".to_string());
        let old_value: Option<String> = None;
        let new_value = Some("new-value".to_string());
        let reason = Some("test".to_string());
        let details = format!(
            "action={} oid={:?} agent={:?} session={:?} old={:?} new={:?} reason={:?}",
            action, oid, agent, session, old_value, new_value, reason
        );
        let prev_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let data_hash = hex::encode(Sha256::digest(details.as_bytes()));
        let chain_hash = hex::encode(Sha256::digest(
            format!("{}:{}:{}", prev_hash, data_hash, created_at).as_bytes(),
        ));
        conn.execute(
            "INSERT INTO audit_log (action, observation_id, agent_id, session_id, old_value, new_value, reason, created_at, prev_hash, data_hash, chain_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![action, oid, agent, session, old_value, new_value, reason, created_at, prev_hash, data_hash, chain_hash],
        )
        .unwrap();
    }

    #[test]
    fn test_run_migrations_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        let (current, applied) = run_migrations(&conn).unwrap();
        assert_eq!(current, 10);
        assert_eq!(applied, 10);
        let status = get_migration_status(&conn).unwrap();
        assert_eq!(status["current_version"], 10);
    }

    #[test]
    fn test_run_migrations_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let (current, applied) = run_migrations(&conn).unwrap();
        assert_eq!(current, 10);
        assert_eq!(applied, 0);
    }

    #[test]
    fn v8_preserves_a_valid_existing_chain() {
        let conn = version_7_db();
        insert_valid_hashed_audit_row(&conn, "update", 1_700_000_000);
        let before: (String, String, String) = conn
            .query_row(
                "SELECT prev_hash, data_hash, chain_hash FROM audit_log WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();

        let (version, applied) = run_migrations(&conn).unwrap();
        let after: (String, String, String) = conn
            .query_row(
                "SELECT prev_hash, data_hash, chain_hash FROM audit_log WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();

        assert_eq!((version, applied), (10, 3));
        assert_eq!(before, after);
    }

    #[test]
    fn v8_backfills_legacy_rows_in_chain_order() {
        let conn = version_7_db();
        conn.execute(
            "INSERT INTO audit_log (action, created_at) VALUES ('legacy-1', 1), ('legacy-2', 2)",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();
        let rows: Vec<(String, String, String)> = conn
            .prepare("SELECT prev_hash, data_hash, chain_hash FROM audit_log ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].0,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert!(!rows[0].1.is_empty());
        assert_eq!(rows[1].0, rows[0].2);
        assert!(!rows[1].1.is_empty());
        assert!(!rows[1].2.is_empty());
    }

    #[test]
    fn migration_failure_rolls_back_prior_backfill_and_version_updates() {
        let conn = version_7_db();
        conn.execute(
            "INSERT INTO audit_log (action, created_at) VALUES ('legacy', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO audit_log (action, created_at, data_hash) VALUES ('tampered', 2, 'not-the-event-hash')",
            [],
        )
        .unwrap();

        assert!(run_migrations(&conn).is_err());
        let version: u32 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        let first_row_data_hash: String = conn
            .query_row("SELECT data_hash FROM audit_log WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, 7);
        assert!(first_row_data_hash.is_empty());
    }

    #[test]
    fn migration_refuses_to_repair_a_disconnected_hashed_chain() {
        let conn = version_7_db();
        insert_valid_hashed_audit_row(&conn, "update-1", 1_700_000_000);
        insert_valid_hashed_audit_row(&conn, "update-2", 1_700_000_001);

        let original: Vec<(String, String, String)> = conn
            .prepare("SELECT prev_hash, data_hash, chain_hash FROM audit_log ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        let forged_prev = "f".repeat(64);
        let forged_chain = hex::encode(Sha256::digest(
            format!("{}:{}:{}", forged_prev, original[1].1, 1_700_000_001).as_bytes(),
        ));
        conn.execute(
            "UPDATE audit_log SET prev_hash = ?1, chain_hash = ?2 WHERE id = 2",
            rusqlite::params![forged_prev, forged_chain],
        )
        .unwrap();

        let tampered: Vec<(String, String, String)> = conn
            .prepare("SELECT prev_hash, data_hash, chain_hash FROM audit_log ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert!(run_migrations(&conn).is_err());

        let version: u32 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        let after: Vec<(String, String, String)> = conn
            .prepare("SELECT prev_hash, data_hash, chain_hash FROM audit_log ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert_eq!(version, 7);
        assert_eq!(after, tampered);
    }

    #[test]
    fn migration_refuses_legacy_rows_after_hashed_history() {
        let conn = version_7_db();
        insert_valid_hashed_audit_row(&conn, "hashed", 1_700_000_000);
        conn.execute(
            "INSERT INTO audit_log (action, created_at) VALUES ('unhashed-after-history', 1700000001)",
            [],
        )
        .unwrap();

        assert!(run_migrations(&conn).is_err());
        let version: u32 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        let fields: (String, String, String) = conn
            .query_row(
                "SELECT prev_hash, data_hash, chain_hash FROM audit_log WHERE id = 2",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(version, 7);
        assert_eq!(
            fields,
            (
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                String::new(),
                String::new(),
            )
        );
    }

    #[test]
    fn concurrent_migration_startup_is_serialized() {
        use std::sync::{Arc, Barrier};
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "synapsis-migration-lock-{}-{unique}.db",
            std::process::id()
        ));
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let conn = Connection::open(&path).unwrap();
                    barrier.wait();
                    run_migrations(&conn).unwrap()
                })
            })
            .collect();

        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert!(results.iter().all(|(version, _)| *version == 10));
        assert_eq!(results.iter().map(|(_, applied)| applied).sum::<u32>(), 10);

        let conn = Connection::open(&path).unwrap();
        let version: u32 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 10);
        drop(conn);
        std::fs::remove_file(&path).unwrap();
    }
}
