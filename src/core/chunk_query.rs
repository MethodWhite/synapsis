//! Chunk Query - Context Chunk Retrieval
//!
//! Implements: mem_chunk_query for efficient context retrieval

use crate::infrastructure::database::Database;
use anyhow::Result;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Context Chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChunk {
    pub chunk_id: String,
    pub project_key: String,
    pub title: String,
    pub content: String,
    pub level: i32,
    pub parent_id: Option<String>,
    pub created_at: i64,
}

/// Chunk Query Manager
pub struct ChunkQueryManager {
    db: Arc<Database>,
}

impl ChunkQueryManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Get chunk by ID
    pub fn get_chunk(&self, chunk_id: &str) -> Result<Option<ContextChunk>> {
        let conn = self.db.get_conn();

        let chunk = conn.query_row(
            "SELECT chunk_id, project_key, title, content, level, parent_id, created_at
             FROM chunks
             WHERE chunk_id = ?1 AND is_active = 1",
            [chunk_id],
            |row: &rusqlite::Row| {
                Ok(ContextChunk {
                    chunk_id: row.get(0)?,
                    project_key: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    level: row.get(4)?,
                    parent_id: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        ).optional()?;

        Ok(chunk)
    }

    /// Get chunks by project
    pub fn get_chunks_by_project(&self, project_key: &str, level: Option<i32>) -> Result<Vec<ContextChunk>> {
        let conn = self.db.get_conn();

        if let Some(l) = level {
            let mut stmt = conn.prepare(
                "SELECT chunk_id, project_key, title, content, level, parent_id, created_at
                 FROM chunks
                 WHERE project_key = ?1 AND level = ?2 AND is_active = 1
                 ORDER BY created_at DESC"
            )?;
            let rows = stmt.query_map([project_key, &l.to_string()], |row: &rusqlite::Row| {
                Ok(ContextChunk {
                    chunk_id: row.get(0)?,
                    project_key: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    level: row.get(4)?,
                    parent_id: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?;
            let chunks: Result<Vec<ContextChunk>, rusqlite::Error> = rows.collect();
            Ok(chunks?)
        } else {
            let mut stmt = conn.prepare(
                "SELECT chunk_id, project_key, title, content, level, parent_id, created_at
                 FROM chunks
                 WHERE project_key = ?1 AND is_active = 1
                 ORDER BY level, created_at DESC"
            )?;
            let rows = stmt.query_map([project_key], |row: &rusqlite::Row| {
                Ok(ContextChunk {
                    chunk_id: row.get(0)?,
                    project_key: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    level: row.get(4)?,
                    parent_id: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?;
            let chunks: Result<Vec<ContextChunk>, rusqlite::Error> = rows.collect();
            Ok(chunks?)
        }
    }

    /// Get context for project (all levels)
    pub fn get_context_for_project(&self, project_key: &str, max_level: i32) -> Result<Vec<ContextChunk>> {
        let conn = self.db.get_conn();

        let mut stmt = conn.prepare(
            "SELECT chunk_id, project_key, title, content, level, parent_id, created_at
             FROM chunks
             WHERE project_key = ?1 AND level <= ?2 AND is_active = 1
             ORDER BY level, created_at DESC",
        )?;

        let chunks = stmt.query_map([project_key, &max_level.to_string()], |row: &rusqlite::Row| {
            Ok(ContextChunk {
                chunk_id: row.get(0)?,
                project_key: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                level: row.get(4)?,
                parent_id: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        Ok(chunks.filter_map(|r: Result<ContextChunk, rusqlite::Error>| r.ok()).collect())
    }
}
