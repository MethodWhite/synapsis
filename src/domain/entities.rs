//! Synapsis Domain Entities

use super::errors::Result;
use super::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub sync_id: SyncId,
    pub session_id: SessionId,
    pub observation_type: ObservationType,
    pub title: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub project: Option<String>,
    pub scope: Scope,
    pub topic_key: Option<String>,
    pub content_hash: ContentHash,
    pub revision_count: i32,
    pub duplicate_count: i32,
    pub last_seen_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deleted_at: Option<Timestamp>,
    pub integrity_hash: Option<String>,
    pub classification: Classification,
}

impl Observation {
    pub fn new(
        session_id: SessionId,
        observation_type: ObservationType,
        title: String,
        content: String,
    ) -> Self {
        let now = Timestamp::now();
        Self {
            id: ObservationId::INVALID,
            sync_id: SyncId::new(),
            session_id,
            observation_type,
            title,
            content,
            tool_name: None,
            project: None,
            scope: Scope::Project,
            topic_key: None,
            content_hash: ContentHash::zero(),
            revision_count: 1,
            duplicate_count: 1,
            last_seen_at: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            integrity_hash: None,
            classification: Classification::Public,
        }
    }
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    pub fn is_active(&self) -> bool {
        self.deleted_at.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub project: String,
    pub directory: String,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub summary: Option<String>,
    pub observation_count: i32,
}

impl Session {
    pub fn new(id: SessionId, project: String, directory: String) -> Self {
        Self {
            id,
            project,
            directory,
            started_at: Timestamp::now(),
            ended_at: None,
            summary: None,
            observation_count: 0,
        }
    }
    pub fn is_active(&self) -> bool {
        self.ended_at.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub id: i64,
    pub sync_id: SyncId,
    pub session_id: SessionId,
    pub content: String,
    pub project: Option<String>,
    pub created_at: Timestamp,
}

impl Prompt {
    pub fn new(session_id: SessionId, content: String, project: Option<String>) -> Self {
        Self {
            id: 0,
            sync_id: SyncId::new(),
            session_id,
            content,
            project,
            created_at: Timestamp::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub observation: Observation,
    pub rank: f64,
    pub highlights: Vec<String>,
}

impl SearchResult {
    pub fn new(observation: Observation, rank: f64) -> Self {
        Self {
            observation,
            rank,
            highlights: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub total_sessions: i64,
    pub total_observations: i64,
    pub total_prompts: i64,
    pub projects: Vec<String>,
    pub active_sessions: i64,
    pub deleted_observations: i64,
    pub storage_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub observation: Observation,
    pub is_focus: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineResult {
    pub focus: Observation,
    pub before: Vec<TimelineEntry>,
    pub after: Vec<TimelineEntry>,
    pub session_info: Option<Session>,
    pub total_in_range: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextOutput {
    pub sessions: Vec<SessionSummary>,
    pub observations: Vec<Observation>,
    pub formatted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub project: String,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub summary: Option<String>,
    pub observation_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddObservationParams {
    pub session_id: SessionId,
    pub observation_type: ObservationType,
    pub title: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub project: Option<String>,
    pub scope: Scope,
    pub topic_key: Option<String>,
}

impl AddObservationParams {
    pub fn validate(&self, max_length: usize) -> Result<()> {
        if self.title.is_empty() {
            return Err(super::errors::SynapsisError::validation_empty("title"));
        }
        if self.content.is_empty() {
            return Err(super::errors::SynapsisError::validation_empty("content"));
        }
        if self.content.len() > max_length {
            return Err(super::errors::SynapsisError::validation_too_large(
                max_length,
                self.content.len(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub obs_type: Option<ObservationType>,
    pub project: Option<String>,
    pub scope: Option<Scope>,
    pub limit: i32,
}

impl SearchParams {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            obs_type: None,
            project: None,
            scope: None,
            limit: 20,
        }
    }
    pub fn with_limit(mut self, limit: i32) -> Self {
        self.limit = limit.min(100).max(1);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub agent_id: String,
    pub session_id: Option<String>,
    pub role: String,
    pub content: String,
    pub token_count: i32,
    pub created_at: u64,
    pub checksum: Option<String>,
}

impl Memory {
    pub fn new(agent_id: String, session_id: Option<String>, role: String, content: String) -> Self {
        use crate::core::uuid::Uuid;
        Self {
            id: Uuid::new_v4().to_hex_string(),
            agent_id,
            session_id,
            role,
            content,
            token_count: 0,
            created_at: 0,
            checksum: None,
        }
    }
}

/// Chunk de contexto para aislamiento por proyecto
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: i64,
    pub chunk_id: ChunkId,
    pub project_key: String,
    pub parent_chunk_id: Option<String>,
    pub title: String,
    pub content: String,
    pub metadata: Option<String>,
    pub size_bytes: i64,
    pub token_count: i32,
    pub level: i32,
    pub is_active: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub pqc_key_hash: Option<String>,
}

/// Contexto global tipo agents.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalContext {
    pub id: i64,
    pub project_key: String,
    pub rules: String,
    pub guidelines: Option<String>,
    pub examples: Option<String>,
    pub variables: Option<String>,
    pub is_active: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
