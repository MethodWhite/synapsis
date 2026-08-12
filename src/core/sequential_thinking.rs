//! Sequential Thinking - Structured Reasoning Trees
//!
//! Implements: think_start, think_step, think_state, think_finish
//! Reasoning trees persist in SQLite, so Synapsis does not depend on an
//! external sequential-thinking MCP server.

use crate::infrastructure::database::Database;
use anyhow::Result;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A reasoning step within a tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingStep {
    pub step_index: i64,
    pub branch: i64,
    pub parent_index: Option<i64>,
    pub thought: String,
    pub created_at: i64,
}

/// A reasoning tree (a full branch-and-bound thought process).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingTree {
    pub tree_id: String,
    pub project: Option<String>,
    pub session_id: Option<String>,
    pub topic: String,
    pub status: String,
    pub steps: Vec<ThinkingStep>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Sequential Thinking Manager
pub struct SequentialThinking {
    db: Arc<Database>,
}

impl SequentialThinking {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn start_tree(
        &self,
        tree_id: &str,
        project: Option<&str>,
        session_id: Option<&str>,
        topic: &str,
    ) -> Result<()> {
        let conn = self.db.get_conn();
        let now = crate::domain::Timestamp::now().0;
        conn.execute(
            "INSERT OR IGNORE INTO thinking_trees (tree_id, project, session_id, topic, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?5)",
            rusqlite::params![tree_id, project, session_id, topic, now],
        )?;
        Ok(())
    }

    pub fn add_step(
        &self,
        tree_id: &str,
        thought: &str,
        branch: i64,
        parent_index: Option<i64>,
    ) -> Result<ThinkingStep> {
        let conn = self.db.get_conn();
        let now = crate::domain::Timestamp::now().0;
        // Compute next step index for this branch.
        let next_index: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(step_index), -1) + 1 FROM thinking_steps WHERE tree_id = ?1 AND branch = ?2",
                rusqlite::params![tree_id, branch],
                |r| r.get(0),
            )
            .unwrap_or(0);

        conn.execute(
            "INSERT INTO thinking_steps (tree_id, step_index, parent_index, branch, thought, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![tree_id, next_index, parent_index, branch, thought, now],
        )?;
        conn.execute(
            "UPDATE thinking_trees SET updated_at = ?1 WHERE tree_id = ?2",
            rusqlite::params![now, tree_id],
        )?;
        Ok(ThinkingStep {
            step_index: next_index,
            branch,
            parent_index,
            thought: thought.to_string(),
            created_at: now,
        })
    }

    pub fn get_tree(&self, tree_id: &str) -> Result<Option<ThinkingTree>> {
        let conn = self.db.get_conn();
        let tree = conn
            .query_row(
                "SELECT tree_id, project, session_id, topic, status, created_at, updated_at
                 FROM thinking_trees WHERE tree_id = ?1",
                [tree_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?;

        let Some((id, project, session_id, topic, status, created_at, updated_at)) = tree else {
            return Ok(None);
        };

        let mut stmt = conn.prepare(
            "SELECT step_index, parent_index, branch, thought, created_at
             FROM thinking_steps WHERE tree_id = ?1 ORDER BY branch ASC, step_index ASC",
        )?;
        let steps = stmt
            .query_map([tree_id], |r| {
                Ok(ThinkingStep {
                    step_index: r.get(0)?,
                    parent_index: r.get(1)?,
                    branch: r.get(2)?,
                    thought: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(Some(ThinkingTree {
            tree_id: id,
            project,
            session_id,
            topic,
            status,
            steps,
            created_at,
            updated_at,
        }))
    }

    pub fn finish_tree(&self, tree_id: &str, status: &str) -> Result<()> {
        let conn = self.db.get_conn();
        let now = crate::domain::Timestamp::now().0;
        conn.execute(
            "UPDATE thinking_trees SET status = ?1, updated_at = ?2 WHERE tree_id = ?3",
            rusqlite::params![status, now, tree_id],
        )?;
        Ok(())
    }

    pub fn list_trees(&self, project: Option<&str>, limit: i32) -> Result<Vec<ThinkingTree>> {
        let conn = self.db.get_conn();
        let tree_ids: Vec<String> = if let Some(p) = project {
            let mut stmt = conn.prepare(
                "SELECT tree_id FROM thinking_trees WHERE project = ?1 ORDER BY updated_at DESC LIMIT ?2",
            )?;
            stmt.query_map(rusqlite::params![p, limit], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT tree_id FROM thinking_trees ORDER BY updated_at DESC LIMIT ?1",
            )?;
            stmt.query_map(rusqlite::params![limit], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect()
        };
        let mut trees = Vec::new();
        for tree_id in tree_ids {
            if let Some(t) = self.get_tree(&tree_id)? {
                trees.push(t);
            }
        }
        Ok(trees)
    }
}
