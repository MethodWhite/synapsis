//! # Context Module - Simplified

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextId(pub String);

impl ContextId {
    pub fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        Self(format!(
            "{:x}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}

impl Default for ContextId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextState {
    Hot,
    Warm,
    Cold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Critical,
    High,
    Normal,
    Low,
    Frozen,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextRef {
    pub id: ContextId,
    pub access_level: AccessLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessLevel {
    None,
    MetadataOnly,
    Summary,
    Partial,
    Full,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IsolationConfig {
    pub global_vars: HashSet<String>,
    pub read_only_vars: HashSet<String>,
    pub isolated_vars: HashSet<String>,
    pub inherit_globals: bool,
}

impl IsolationConfig {
    pub fn new() -> Self {
        Self {
            global_vars: HashSet::new(),
            read_only_vars: HashSet::new(),
            isolated_vars: HashSet::new(),
            inherit_globals: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextMetrics {
    pub access_count: u64,
    pub last_access: i64,
    pub hot_score: f64,
    pub memory_size: usize,
    pub connections_count: usize,
}

impl ContextMetrics {
    pub fn new() -> Self {
        Self {
            access_count: 0,
            last_access: now_ts(),
            hot_score: 1.0,
            memory_size: 0,
            connections_count: 0,
        }
    }
    pub fn touch(&mut self) {
        self.access_count += 1;
        self.last_access = now_ts();
    }
}

pub type Timestamp = i64;

pub fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<ContextValue>),
    Object(Vec<(String, ContextValue)>),
    Null,
}

impl ContextValue {
    pub fn estimated_size(&self) -> usize {
        match self {
            ContextValue::String(s) => s.len(),
            ContextValue::Number(_) => 8,
            ContextValue::Boolean(_) => 1,
            ContextValue::Array(a) => a.iter().map(|v| v.estimated_size()).sum(),
            ContextValue::Object(o) => o.iter().map(|(k, v)| k.len() + v.estimated_size()).sum(),
            ContextValue::Null => 0,
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        match self {
            ContextValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            ContextValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ContextValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<ContextValue>> {
        match self {
            ContextValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Vec<(String, ContextValue)>> {
        match self {
            ContextValue::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, ContextValue::Null)
    }
}

impl From<String> for ContextValue {
    fn from(s: String) -> Self {
        ContextValue::String(s)
    }
}
impl From<&str> for ContextValue {
    fn from(s: &str) -> Self {
        ContextValue::String(s.to_string())
    }
}
impl From<i64> for ContextValue {
    fn from(n: i64) -> Self {
        ContextValue::Number(n as f64)
    }
}
impl From<f64> for ContextValue {
    fn from(n: f64) -> Self {
        ContextValue::Number(n)
    }
}
impl From<bool> for ContextValue {
    fn from(b: bool) -> Self {
        ContextValue::Boolean(b)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub id: ContextId,
    pub name: String,
    pub context_type: ContextType,
    pub state: ContextState,
    pub priority: Priority,
    pub variables: std::collections::HashMap<String, ContextValue>,
    pub connections: HashSet<ContextRef>,
    pub metadata: std::collections::HashMap<String, String>,
    pub isolation: IsolationConfig,
    pub metrics: ContextMetrics,
    pub summary: String,
    pub created_at: Timestamp,
    pub tags: HashSet<String>,
    pub parent: Option<ContextId>,
}

impl Context {
    pub fn new(name: String, context_type: ContextType) -> Self {
        Self {
            id: ContextId::new(),
            name,
            context_type,
            state: ContextState::Hot,
            priority: Priority::Normal,
            variables: std::collections::HashMap::new(),
            connections: HashSet::new(),
            metadata: std::collections::HashMap::new(),
            isolation: IsolationConfig::new(),
            metrics: ContextMetrics::new(),
            summary: String::new(),
            created_at: now_ts(),
            tags: HashSet::new(),
            parent: None,
        }
    }

    pub fn set_var(&mut self, name: &str, value: ContextValue) {
        self.variables.insert(name.to_string(), value);
    }

    pub fn get_var(&self, name: &str) -> Option<&ContextValue> {
        self.variables.get(name)
    }

    pub fn touch(&mut self) {
        self.metrics.touch();
        if self.state != ContextState::Hot {
            self.state = ContextState::Hot;
        }
    }

    pub fn memory_size(&self) -> usize {
        let vars: usize = self.variables.values().map(|v| v.estimated_size()).sum();
        let meta: usize = self.metadata.values().map(|s| s.len()).sum();
        std::mem::size_of_val(self) + vars + meta + self.summary.len()
    }

    pub fn generate_summary(&self) -> String {
        if !self.summary.is_empty() {
            return self.summary.clone();
        }
        format!(
            "[{:?}] {} - {} vars, {} connections",
            self.context_type,
            self.name,
            self.variables.len(),
            self.connections.len()
        )
    }
}

pub struct ContextRegistry {
    hot_contexts: std::collections::HashMap<ContextId, Context>,
    warm_contexts: std::collections::HashMap<ContextId, Context>,
    cold_refs: std::collections::HashMap<ContextId, ColdRef>,
    global_vars: std::collections::HashMap<String, ContextValue>,
    config: RegistryConfig,
    working_set: std::collections::HashSet<ContextId>,
}

struct ColdRef {
    archived_at: Timestamp,
    priority: Priority,
    size_bytes: usize,
}

struct RegistryConfig {
    max_hot: usize,
    max_warm: usize,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            max_hot: 10,
            max_warm: 50,
        }
    }
}

impl ContextRegistry {
    pub fn new() -> Self {
        Self {
            hot_contexts: std::collections::HashMap::new(),
            warm_contexts: std::collections::HashMap::new(),
            cold_refs: std::collections::HashMap::new(),
            global_vars: std::collections::HashMap::new(),
            config: RegistryConfig::default(),
            working_set: std::collections::HashSet::new(),
        }
    }

    pub fn create(&mut self, name: String, ctx_type: ContextType) -> ContextId {
        let ctx = Context::new(name, ctx_type);
        let id = ctx.id.clone();
        self.hot_contexts.insert(id.clone(), ctx);
        self.working_set.insert(id.clone());
        id
    }

    pub fn create_child(
        &mut self,
        name: String,
        ctx_type: ContextType,
        parent: &ContextId,
    ) -> ContextId {
        let mut ctx = Context::new(name, ctx_type);
        ctx.parent = Some(parent.clone());
        ctx.connections.insert(ContextRef {
            id: parent.clone(),
            access_level: AccessLevel::Summary,
        });
        let id = ctx.id.clone();
        self.hot_contexts.insert(id.clone(), ctx);
        self.working_set.insert(id.clone());
        id
    }

    pub fn get(&self, id: &ContextId) -> Option<&Context> {
        self.hot_contexts
            .get(id)
            .or_else(|| self.warm_contexts.get(id))
    }

    pub fn get_mut(&mut self, id: &ContextId) -> Option<&mut Context> {
        self.hot_contexts
            .get_mut(id)
            .or_else(|| self.warm_contexts.get_mut(id))
    }

    pub fn touch(&mut self, id: &ContextId) {
        self.working_set.insert(id.clone());
        if let Some(ctx) = self.get_mut(id) {
            ctx.touch();
        }
        if self.warm_contexts.contains_key(id) {
            if let Some(ctx) = self.warm_contexts.remove(id) {
                self.hot_contexts.insert(id.clone(), ctx);
            }
        }
    }

    pub fn set_global(&mut self, name: &str, value: ContextValue) {
        self.global_vars.insert(name.to_string(), value);
    }

    pub fn get_global(&self, name: &str) -> Option<&ContextValue> {
        self.global_vars.get(name)
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let q = query.to_lowercase();
        let mut results = Vec::new();

        for (id, ctx) in &self.hot_contexts {
            let rel = self.calc_relevance(ctx, &q);
            if rel > 0.0 {
                results.push(SearchResult {
                    context_id: id.clone(),
                    name: ctx.name.clone(),
                    context_type: ctx.context_type,
                    relevance: rel,
                    state: ContextState::Hot,
                });
            }
        }

        for (id, ctx) in &self.warm_contexts {
            let rel = self.calc_relevance(ctx, &q);
            if rel > 0.0 {
                results.push(SearchResult {
                    context_id: id.clone(),
                    name: ctx.name.clone(),
                    context_type: ctx.context_type,
                    relevance: rel,
                    state: ContextState::Warm,
                });
            }
        }

        results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());
        results
    }

    fn calc_relevance(&self, ctx: &Context, query: &str) -> f64 {
        let mut score: f64 = 0.0;
        if ctx.name.to_lowercase().contains(query) {
            score += 0.5;
        }
        if ctx.summary.to_lowercase().contains(query) {
            score += 0.3;
        }
        for tag in &ctx.tags {
            if tag.to_lowercase().contains(query) {
                score += 0.2;
                break;
            }
        }
        score.min(1.0)
    }

    pub fn stats(&self) -> RegistryStats {
        RegistryStats {
            hot: self.hot_contexts.len(),
            warm: self.warm_contexts.len(),
            cold: self.cold_refs.len(),
            working_set: self.working_set.len(),
        }
    }

    pub fn list(&self) -> Vec<ContextInfo> {
        let mut infos = Vec::new();
        for (id, ctx) in &self.hot_contexts {
            infos.push(ContextInfo {
                context_id: id.clone(),
                name: ctx.name.clone(),
                context_type: ctx.context_type,
                state: ContextState::Hot,
                priority: ctx.priority,
                size: ctx.memory_size(),
            });
        }
        for (id, ctx) in &self.warm_contexts {
            infos.push(ContextInfo {
                context_id: id.clone(),
                name: ctx.name.clone(),
                context_type: ctx.context_type,
                state: ContextState::Warm,
                priority: ctx.priority,
                size: ctx.memory_size(),
            });
        }
        infos
    }
}

impl Default for ContextRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct RegistryStats {
    pub hot: usize,
    pub warm: usize,
    pub cold: usize,
    pub working_set: usize,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub context_id: ContextId,
    pub name: String,
    pub context_type: ContextType,
    pub relevance: f64,
    pub state: ContextState,
}

#[derive(Debug, Clone)]
pub struct ContextInfo {
    pub context_id: ContextId,
    pub name: String,
    pub context_type: ContextType,
    pub state: ContextState,
    pub priority: Priority,
    pub size: usize,
}
