//! Multi-Agent Safe Database with Session Validation

use crate::infrastructure::database::Database;
use std::sync::{Arc, RwLock};
use anyhow::Result;

pub struct MultiAgentDatabase {
    db: Arc<Database>,
    agent_locks: Arc<RwLock<std::collections::HashMap<String, Arc<RwLock<()>>>>>,
}

impl MultiAgentDatabase {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            agent_locks: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Execute operation with agent-specific lock and session validation
    pub fn with_agent<F, T>(&self, agent_id: &str, op: F) -> Result<T>
    where
        F: FnOnce(&Database) -> Result<T>,
    {
        // Verify session is active before allowing operations
        if let Ok(conn) = self.db.conn.lock() {
            let stmt_result = conn.prepare("SELECT is_active FROM agent_sessions WHERE id = ? AND is_active = 1");
            if let Ok(mut stmt) = stmt_result {
                let result: Result<i32, rusqlite::Error> = stmt.query_row([agent_id], |row| row.get::<_, i32>(0));
                match result {
                    Ok(is_active) => {
                        if is_active != 1 {
                            return Err(anyhow::anyhow!("Session inactive or not found"));
                        }
                    }
                    Err(_) => {
                        return Err(anyhow::anyhow!("Session not found"));
                    }
                }
            }
        }

        let lock = {
            let mut locks = self.agent_locks.write().unwrap();
            locks
                .entry(agent_id.into())
                .or_insert_with(|| Arc::new(RwLock::new(())))
                .clone()
        };
        let _guard = lock.write().unwrap();
        op(&self.db)
    }

    /// Acquire lock with session validation
    pub fn acquire_lock(&self, session_id: &str, lock_key: &str, resource_type: &str, resource_id: Option<&str>, ttl_secs: i64) -> Result<bool> {
        // Verify session is active before allowing lock acquisition
        if let Ok(conn) = self.db.conn.lock() {
            let stmt_result = conn.prepare("SELECT is_active FROM agent_sessions WHERE id = ? AND is_active = 1");
            if let Ok(mut stmt) = stmt_result {
                let result: Result<i32, rusqlite::Error> = stmt.query_row([session_id], |row| row.get::<_, i32>(0));
                match result {
                    Ok(is_active) => {
                        if is_active != 1 {
                            return Err(anyhow::anyhow!("Cannot acquire lock: Session is inactive"));
                        }
                    }
                    Err(_) => {
                        return Err(anyhow::anyhow!("Cannot acquire lock: Session not found"));
                    }
                }
            }
        }

        // Proceed with normal lock acquisition
        self.db.acquire_lock(session_id, lock_key, resource_type, resource_id, ttl_secs)
            .map_err(|e| anyhow::anyhow!("Lock acquisition failed: {}", e))
    }
}

impl Clone for MultiAgentDatabase {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            agent_locks: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}
