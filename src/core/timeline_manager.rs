//! Timeline Manager - Complete Timeline with Context
//!
//! Implements: mem_timeline with focus entry and surrounding context

use crate::infrastructure::database::Database;
use anyhow::Result;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Timeline Entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub observation_id: i64,
    pub title: String,
    pub observation_type: String,
    pub created_at: i64,
    pub is_focus: bool,
}

/// Timeline Result with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineResult {
    pub focus: Option<TimelineEntry>,
    pub before: Vec<TimelineEntry>,
    pub after: Vec<TimelineEntry>,
    pub total_in_range: i64,
}

/// Timeline Manager
pub struct TimelineManager {
    db: Arc<Database>,
}

impl TimelineManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Get simple timeline
    pub fn get_timeline(&self, limit: i32) -> Result<Vec<TimelineEntry>> {
        let conn = self.db.get_conn();

        let mut stmt = conn.prepare(
            "SELECT id, title, observation_type, created_at
             FROM observations
             WHERE deleted_at IS NULL
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let entries = stmt.query_map([limit], |row: &rusqlite::Row| {
            Ok(TimelineEntry {
                observation_id: row.get(0)?,
                title: row.get(1)?,
                observation_type: row.get(2)?,
                created_at: row.get(3)?,
                is_focus: false,
            })
        })?;

        Ok(entries.filter_map(|r: Result<TimelineEntry, rusqlite::Error>| r.ok()).collect())
    }

    /// Get timeline with focus and context
    pub fn get_timeline_with_context(&self, focus_id: i64, before: i32, after: i32) -> Result<TimelineResult> {
        let conn = self.db.get_conn();

        // Get focus entry
        let focus = conn.query_row(
            "SELECT id, title, observation_type, created_at
             FROM observations
             WHERE id = ?1 AND deleted_at IS NULL",
            [focus_id],
            |row: &rusqlite::Row| {
                Ok(TimelineEntry {
                    observation_id: row.get(0)?,
                    title: row.get(1)?,
                    observation_type: row.get(2)?,
                    created_at: row.get(3)?,
                    is_focus: true,
                })
            },
        ).optional()?;

        // Get entries before focus
        let before_entries = self.get_entries_before(&conn, focus_id, before)?;

        // Get entries after focus
        let after_entries = self.get_entries_after(&conn, focus_id, after)?;

        // Get total count in range
        let total = conn.query_row(
            "SELECT COUNT(*) FROM observations WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )?;

        Ok(TimelineResult {
            focus,
            before: before_entries,
            after: after_entries,
            total_in_range: total,
        })
    }

    fn get_entries_before(&self, conn: &rusqlite::Connection, focus_id: i64, limit: i32) -> Result<Vec<TimelineEntry>> {
        let mut stmt = conn.prepare(
            "SELECT id, title, observation_type, created_at
             FROM observations
             WHERE deleted_at IS NULL
             AND created_at < (SELECT created_at FROM observations WHERE id = ?1)
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let entries = stmt.query_map([focus_id, limit.into()], |row: &rusqlite::Row| {
            Ok(TimelineEntry {
                observation_id: row.get(0)?,
                title: row.get(1)?,
                observation_type: row.get(2)?,
                created_at: row.get(3)?,
                is_focus: false,
            })
        })?;

        Ok(entries.filter_map(|r: Result<TimelineEntry, rusqlite::Error>| r.ok()).collect())
    }

    fn get_entries_after(&self, conn: &rusqlite::Connection, focus_id: i64, limit: i32) -> Result<Vec<TimelineEntry>> {
        let mut stmt = conn.prepare(
            "SELECT id, title, observation_type, created_at
             FROM observations
             WHERE deleted_at IS NULL
             AND created_at > (SELECT created_at FROM observations WHERE id = ?1)
             ORDER BY created_at ASC
             LIMIT ?2",
        )?;

        let entries = stmt.query_map([focus_id, limit.into()], |row: &rusqlite::Row| {
            Ok(TimelineEntry {
                observation_id: row.get(0)?,
                title: row.get(1)?,
                observation_type: row.get(2)?,
                created_at: row.get(3)?,
                is_focus: false,
            })
        })?;

        Ok(entries.filter_map(|r: Result<TimelineEntry, rusqlite::Error>| r.ok()).collect())
    }
}
