//! # Context Types
//!
//! Tipos fundamentales para el sistema de contextos.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Identificador único de contexto (usa String internamente para serialización)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextId(pub String);

impl ContextId {
    pub fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(format!("{:x}", ts))
    }
}

impl Default for ContextId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ContextId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Ord for ContextId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for ContextId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Estado térmico del contexto
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextState {
    Hot = 0,
    Warm = 1,
    Cold = 2,
}

impl ContextState {
    pub fn next(&self) -> Self {
        match self {
            ContextState::Hot => ContextState::Warm,
            ContextState::Warm => ContextState::Cold,
            ContextState::Cold => ContextState::Cold,
        }
    }

    pub fn promote(&mut self) {
        match self {
            ContextState::Cold => *self = ContextState::Warm,
            ContextState::Warm => *self = ContextState::Hot,
            ContextState::Hot => {}
        }
    }

    pub fn demote(&mut self) {
        *self = self.next();
    }
}

/// Prioridad del contexto
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Frozen = 4,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Tipo de contexto
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextType {
    Session,
    Project,
    Task,
    Conversation,
    System,
}

impl Default for ContextType {
    fn default() -> Self {
        ContextType::Session
    }
}

/// Referencia a otro contexto
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextRef {
    pub id: ContextId,
    pub relation: ContextRelation,
    pub access_level: AccessLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextRelation {
    Parent,
    Child,
    Sibling,
    Referenced,
    DependsOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Serialize, Deserialize)]
pub enum AccessLevel {
    None,
    MetadataOnly,
    Summary,
    Partial,
    Full,
}

/// Configuración de aislamiento
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolationConfig {
    pub global_vars: HashSet<String>,
    pub read_only_vars: HashSet<String>,
    pub isolated_vars: HashSet<String>,
    pub inherit_globals: bool,
    pub max_access_from_connections: AccessLevel,
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            global_vars: HashSet::new(),
            read_only_vars: HashSet::new(),
            isolated_vars: HashSet::new(),
            inherit_globals: true,
            max_access_from_connections: AccessLevel::Summary,
        }
    }
}

/// Métricas de uso del contexto
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextMetrics {
    pub access_count: u64,
    pub last_access: i64,
    pub relevance_score: f64,
    pub memory_size: usize,
    pub connections_count: usize,
    pub hot_score: f64,
}

impl ContextMetrics {
    pub fn new() -> Self {
        Self {
            access_count: 0,
            last_access: now_timestamp(),
            relevance_score: 0.0,
            memory_size: 0,
            connections_count: 0,
            hot_score: 0.0,
        }
    }

    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.last_access = now_timestamp();
    }

    pub fn calculate_hot_score(&mut self, now: i64) {
        let time_since_access = (now - self.last_access) as f64;
        let recency = (-time_since_access / 3600.0).exp();
        let frequency = (self.access_count as f64).sqrt();
        let connection_boost = (self.connections_count as f64) * 0.1;

        self.hot_score = recency * (1.0 + frequency * 0.1 + connection_boost);
    }
}

/// Timestamp helper
pub type Timestamp = i64;

pub fn now_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
