//! Session Context Persistence
//!
//! Persists conversation context and session state to prevent loss between restarts.
//! Uses the context_cache table to store serialized session context.

use crate::infrastructure::database::Database;
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Session context data that can be serialized and stored
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    /// Session ID this context belongs to
    pub session_id: String,
    /// Serialized conversation history (format depends on consumer)
    pub conversation_history: String,
    /// Additional metadata (JSON string)
    pub metadata: String,
    /// Model state or other serialized data
    pub state_data: Vec<u8>,
    /// Timestamp of last update
    pub updated_at: i64,
}

/// Manager for persisting and restoring session context
pub struct SessionContextManager {
    db: Arc<Database>,
}

impl SessionContextManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Save or update session context
    pub fn save_context(&self, context: &SessionContext) -> Result<()> {
        let conn = self.db.get_conn();

        let serialized = serde_json::to_string(context)?;
        let now = self.current_timestamp();

        // Check if context already exists
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM context_cache WHERE cache_key = ?1",
                [format!("session_context:{}", context.session_id)],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(_) = exists {
            // Update existing
            conn.execute(
                "UPDATE context_cache SET 
                    data = ?1,
                    last_accessed = ?2,
                    hits = hits + 1
                 WHERE cache_key = ?3",
                params![
                    serialized,
                    now,
                    format!("session_context:{}", context.session_id),
                ],
            )?;
        } else {
            // Insert new
            conn.execute(
                "INSERT INTO context_cache (cache_key, project_key, data, created_at, last_accessed, hits)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1)",
                params![
                    format!("session_context:{}", context.session_id),
                    format!("session:{}", context.session_id),
                    serialized,
                    now,
                    now,
                ],
            )?;
        }

        Ok(())
    }

    /// Load session context by session ID
    pub fn load_context(&self, session_id: &str) -> Result<Option<SessionContext>> {
        let conn = self.db.get_conn();

        let cache_key = format!("session_context:{}", session_id);

        let context_json: Option<String> = conn
            .query_row(
                "SELECT data FROM context_cache WHERE cache_key = ?1",
                [&cache_key],
                |row| row.get(0),
            )
            .optional()?;

        match context_json {
            Some(json) => {
                // Update last_accessed and hits
                let now = self.current_timestamp();
                conn.execute(
                    "UPDATE context_cache SET last_accessed = ?1, hits = hits + 1 WHERE cache_key = ?2",
                    params![now, &cache_key],
                )?;

                let context: SessionContext = serde_json::from_str(&json)?;
                Ok(Some(context))
            }
            None => Ok(None),
        }
    }

    /// Delete session context
    pub fn delete_context(&self, session_id: &str) -> Result<()> {
        let conn = self.db.get_conn();

        let cache_key = format!("session_context:{}", session_id);

        conn.execute(
            "DELETE FROM context_cache WHERE cache_key = ?1",
            [&cache_key],
        )?;

        Ok(())
    }

    /// List all saved session contexts
    pub fn list_contexts(&self) -> Result<Vec<SessionContext>> {
        let conn = self.db.get_conn();

        let mut stmt = conn.prepare(
            "SELECT data FROM context_cache WHERE cache_key LIKE 'session_context:%' ORDER BY last_accessed DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let json: String = row.get(0)?;
            Ok(json)
        })?;

        let mut contexts = Vec::new();
        for row in rows {
            let json = row?;
            if let Ok(context) = serde_json::from_str(&json) {
                contexts.push(context);
            }
        }

        Ok(contexts)
    }

    /// Update conversation history for a session
    pub fn update_conversation_history(&self, session_id: &str, history: &str) -> Result<()> {
        let existing = self.load_context(session_id)?;

        let context = match existing {
            Some(mut ctx) => {
                ctx.conversation_history = history.to_string();
                ctx.updated_at = self.current_timestamp();
                ctx
            }
            None => SessionContext {
                session_id: session_id.to_string(),
                conversation_history: history.to_string(),
                metadata: "{}".to_string(),
                state_data: Vec::new(),
                updated_at: self.current_timestamp(),
            },
        };

        self.save_context(&context)
    }

    /// Update metadata for a session
    pub fn update_metadata(&self, session_id: &str, metadata: &str) -> Result<()> {
        let existing = self.load_context(session_id)?;

        let context = match existing {
            Some(mut ctx) => {
                ctx.metadata = metadata.to_string();
                ctx.updated_at = self.current_timestamp();
                ctx
            }
            None => SessionContext {
                session_id: session_id.to_string(),
                conversation_history: "".to_string(),
                metadata: metadata.to_string(),
                state_data: Vec::new(),
                updated_at: self.current_timestamp(),
            },
        };

        self.save_context(&context)
    }

    /// Update binary state data for a session
    pub fn update_state_data(&self, session_id: &str, state_data: Vec<u8>) -> Result<()> {
        let existing = self.load_context(session_id)?;

        let context = match existing {
            Some(mut ctx) => {
                ctx.state_data = state_data;
                ctx.updated_at = self.current_timestamp();
                ctx
            }
            None => SessionContext {
                session_id: session_id.to_string(),
                conversation_history: "".to_string(),
                metadata: "{}".to_string(),
                state_data,
                updated_at: self.current_timestamp(),
            },
        };

        self.save_context(&context)
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
    fn test_session_context_serialization() {
        let context = SessionContext {
            session_id: "test-session-123".to_string(),
            conversation_history: r#"["Hello", "World"]"#.to_string(),
            metadata: r#"{"model": "llama3"}"#.to_string(),
            state_data: vec![1, 2, 3, 4],
            updated_at: 1234567890,
        };

        let serialized = serde_json::to_string(&context).unwrap();
        let deserialized: SessionContext = serde_json::from_str(&serialized).unwrap();

        assert_eq!(context.session_id, deserialized.session_id);
        assert_eq!(
            context.conversation_history,
            deserialized.conversation_history
        );
        assert_eq!(context.metadata, deserialized.metadata);
        assert_eq!(context.state_data, deserialized.state_data);
        assert_eq!(context.updated_at, deserialized.updated_at);
    }
}
