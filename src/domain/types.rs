//! Synapsis Domain Types

use crate::core::uuid::Uuid;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ObservationId(pub i64);

impl ObservationId {
    pub const INVALID: Self = Self(0);
    pub const fn new(id: i64) -> Self {
        Self(id)
    }
    pub const fn is_valid(self) -> bool {
        self.0 > 0
    }
}

impl Default for ObservationId {
    fn default() -> Self {
        Self::INVALID
    }
}
impl From<i64> for ObservationId {
    fn from(v: i64) -> Self {
        Self(v)
    }
}
impl From<ObservationId> for i64 {
    fn from(v: ObservationId) -> Self {
        v.0
    }
}
impl std::fmt::Display for ObservationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ToSql for ObservationId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0))
    }
}

impl FromSql for ObservationId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(ObservationId)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    pub fn manual(project: &str) -> Self {
        Self(format!("manual-{}", project))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SessionId {
    fn from(v: String) -> Self {
        Self(v)
    }
}
impl From<&str> for SessionId {
    fn from(v: &str) -> Self {
        Self(v.to_string())
    }
}
impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum ObservationType {
    #[default]
    Manual = 0,
    ToolUse = 1,
    FileChange = 2,
    Command = 3,
    FileRead = 4,
    Search = 5,
    Decision = 6,
    Architecture = 7,
    Bugfix = 8,
    Pattern = 9,
    Config = 10,
    Discovery = 11,
    Learning = 12,
}

impl ObservationType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "tool_use" | "tooluse" => Self::ToolUse,
            "file_change" => Self::FileChange,
            "command" => Self::Command,
            "file_read" => Self::FileRead,
            "search" => Self::Search,
            "decision" => Self::Decision,
            "architecture" => Self::Architecture,
            "bugfix" => Self::Bugfix,
            "pattern" => Self::Pattern,
            "config" => Self::Config,
            "discovery" => Self::Discovery,
            "learning" => Self::Learning,
            _ => Self::Manual,
        }
    }
}

impl std::fmt::Display for ObservationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum Scope {
    #[default]
    Project = 0,
    Personal = 1,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Timestamp(pub i64);

impl Timestamp {
    pub const UNIX_EPOCH: Self = Self(0);
    pub fn now() -> Self {
        Self(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        )
    }
}

impl From<i64> for Timestamp {
    fn from(v: i64) -> Self {
        Self(v)
    }
}
impl From<Timestamp> for i64 {
    fn from(v: Timestamp) -> Self {
        v.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct SyncId(pub String);

impl SyncId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_hex_string())
    }
}
impl From<String> for SyncId {
    fn from(v: String) -> Self {
        Self(v)
    }
}
impl From<&str> for SyncId {
    fn from(v: &str) -> Self {
        Self(v.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum Classification {
    #[default]
    Public = 0,
    Internal = 1,
    Confidential = 2,
    Secret = 3,
    TopSecret = 4,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    pub const SIZE: usize = 32;
    pub fn zero() -> Self {
        Self([0u8; 32])
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ChunkId(pub String);

impl ChunkId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_hex_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub chunk_id: ChunkId,
    pub size: usize,
    pub hash: ContentHash,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ManifestId(pub String);

impl ManifestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_hex_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_sync: u64,
    pub commit_count: u64,
    pub pending_changes: bool,
    pub conflict_detected: bool,
    pub circuit_breaker_state: String,
}

// SQLite conversions for ObservationType
impl ToSql for ObservationType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(*self as u8))
    }
}

impl FromSql for ObservationType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let val = u8::column_result(value)?;
        Ok(match val {
            1 => Self::ToolUse,
            2 => Self::FileChange,
            3 => Self::Command,
            4 => Self::FileRead,
            5 => Self::Search,
            6 => Self::Decision,
            7 => Self::Architecture,
            8 => Self::Bugfix,
            9 => Self::Pattern,
            10 => Self::Config,
            11 => Self::Discovery,
            12 => Self::Learning,
            _ => Self::Manual,
        })
    }
}

// SQLite conversions for Scope
impl ToSql for Scope {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(*self as u8))
    }
}

impl FromSql for Scope {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let val = u8::column_result(value)?;
        Ok(match val {
            1 => Self::Personal,
            _ => Self::Project,
        })
    }
}

// SQLite conversions for Classification
impl ToSql for Classification {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(*self as u8))
    }
}

impl FromSql for Classification {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let val = u8::column_result(value)?;
        Ok(match val {
            1 => Self::Internal,
            2 => Self::Confidential,
            3 => Self::Secret,
            4 => Self::TopSecret,
            _ => Self::Public,
        })
    }
}

// SQLite conversions for Timestamp
impl ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0))
    }
}

impl FromSql for Timestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Timestamp)
    }
}

// SQLite conversions for ContentHash
impl ToSql for ContentHash {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.as_slice()))
    }
}

impl FromSql for ContentHash {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Blob(bytes) => {
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(bytes);
                    Ok(ContentHash(arr))
                } else {
                    Err(FromSqlError::InvalidType)
                }
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub priority: u8,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    pub task_id: String,
    pub node_id: String,
    pub output: String,
    pub success: bool,
}
