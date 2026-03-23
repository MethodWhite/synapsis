//! Synapsis Domain Ports

use super::entities::SearchParams;
use super::errors::{Result, SynapsisError};
use super::types::*;

pub trait StoragePort: Send + Sync {
    fn init(&self) -> Result<()>;
    fn get_observation(&self, id: ObservationId) -> Result<Option<super::entities::Observation>>;
    fn save_observation(&self, obs: &super::entities::Observation) -> Result<ObservationId>;
    fn search_observations(
        &self,
        params: &SearchParams,
    ) -> Result<Vec<super::entities::SearchResult>>;
    fn get_timeline(&self, limit: i32) -> Result<Vec<super::entities::TimelineEntry>>;
}

pub trait SessionPort: Send + Sync {
    fn start_session(&self, project: &str, directory: &str) -> Result<SessionId>;
    fn end_session(&self, id: &SessionId, summary: Option<String>) -> Result<()>;
    fn list_sessions(&self) -> Result<Vec<super::entities::SessionSummary>>;
}

pub trait SyncPort: Send + Sync {
    fn get_status(&self) -> Result<SyncStatus>;
}

pub trait MemoryPort: Send + Sync {
    fn save_memory(&self, memory: &super::entities::Memory) -> Result<()>;
    fn get_memories(&self, agent_id: &str, session_id: Option<&str>) -> Result<Vec<super::entities::Memory>>;
    fn clear_memories(&self, agent_id: &str, session_id: Option<&str>) -> Result<()>;
}

pub trait ExportPort: Send + Sync {
    fn export_all(&self) -> Result<String>;
    fn import_data(&self, data: &str) -> Result<i64>;
}

#[derive(Debug, Clone)]
pub enum AuditOperation {
    Create,
    Read,
    Update,
    Delete,
    Search,
    SessionStart,
    SessionEnd,
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: Timestamp,
    pub operation: AuditOperation,
    pub resource: String,
    pub agent_id: Option<String>,
}

pub fn from_json<T: serde::de::DeserializeOwned>(json: &str) -> Result<T> {
    serde_json::from_str(json).map_err(|_| SynapsisError::validation_malformed_json())
}

pub fn to_json<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(|e| SynapsisError::internal_bug(e.to_string()))
}
