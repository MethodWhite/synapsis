//! # Context Registry
//!
//! Registro central de contextos con gestión inteligente.
//! A diferencia de Engram:
//! - NO carga todo en memoria
//! - Aislamiento real entre contextos
//! - Reciclaje inteligente en frío
//! - Carga perezosa y prefetch

use super::cold_storage::{ColdStats, ColdStorage};
use super::context::{Context, ContextValue};
use super::global_context::{GlobalContext, VarType};
use super::relevance::{ContextRelevanceData, RelevanceEngine};
use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Registro principal de contextos
pub struct ContextRegistry {
    /// Contextos activos en memoria
    hot_contexts: HashMap<ContextId, Context>,

    /// Contextos tibios (en memoria pero no activos)
    warm_contexts: HashMap<ContextId, Context>,

    /// Referencias a contextos archivados (índice solamente)
    cold_refs: HashMap<ContextId, ColdRef>,

    /// Contexto global
    global: GlobalContext,

    /// Motor de relevancia
    relevance: RelevanceEngine,

    /// Almacenamiento frío
    cold_storage: ColdStorage,

    /// Configuración
    config: RegistryConfig,

    /// Working set (contextos actualmente en uso)
    working_set: HashSet<ContextId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ColdRef {
    pub context_id: ContextId,
    pub archived_at: Timestamp,
    pub priority: Priority,
    pub size_bytes: usize,
}

#[derive(Debug, Clone)]
struct RegistryConfig {
    pub max_hot_contexts: usize,
    pub max_warm_contexts: usize,
    pub eviction_threshold: f64,
    pub prefetch_enabled: bool,
    pub prefetch_count: usize,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            max_hot_contexts: 10,
            max_warm_contexts: 50,
            eviction_threshold: 0.2,
            prefetch_enabled: true,
            prefetch_count: 3,
        }
    }
}

impl ContextRegistry {
    pub fn new(data_dir: PathBuf) -> Self {
        let cold_path = data_dir.join("cold_storage");

        Self {
            hot_contexts: HashMap::new(),
            warm_contexts: HashMap::new(),
            cold_refs: HashMap::new(),
            global: GlobalContext::new(),
            relevance: RelevanceEngine::new(),
            cold_storage: ColdStorage::new(cold_path),
            config: RegistryConfig::default(),
            working_set: HashSet::new(),
        }
    }

    /// Crea un nuevo contexto
    pub fn create(&mut self, name: String, context_type: ContextType) -> ContextId {
        let mut context = Context::new(name, context_type);
        let id = context.id.clone();

        self.hot_contexts.insert(id.clone(), context);
        self.relevance.record_access(&id);

        id
    }

    /// Crea un contexto hijo (aislado del padre pero conectado)
    pub fn create_child(
        &mut self,
        name: String,
        context_type: ContextType,
        parent_id: &ContextId,
    ) -> ContextId {
        let mut context = Context::new(name, context_type);
        let id = context.id.clone();

        // Conectar con padre
        context.connect(ContextRef {
            id: parent_id.clone(),
            relation: ContextRelation::Parent,
            access_level: AccessLevel::Partial,
        });

        // Añadir referencia inversa al padre si existe
        if let Some(parent) = self.get_context_mut(parent_id) {
            parent.connect(ContextRef {
                id: id.clone(),
                relation: ContextRelation::Child,
                access_level: AccessLevel::Summary,
            });
        }

        self.hot_contexts.insert(id.clone(), context);
        self.relevance.record_access(&id);

        id
    }

    /// Obtiene un contexto (carga si es necesario)
    pub fn get(&mut self, id: &ContextId) -> Option<&Context> {
        // Primero buscar en hot
        if let Some(ctx) = self.hot_contexts.get(id) {
            return Some(ctx);
        }

        // Luego en warm
        if let Some(ctx) = self.warm_contexts.get(id) {
            return Some(ctx);
        }

        // Cargar desde frío si es necesario
        if self.working_set.contains(id) {
            self.restore_from_cold(id);
            return self.hot_contexts.get(id);
        }

        None
    }

    /// Obtiene contexto mutable
    pub fn get_context_mut(&mut self, id: &ContextId) -> Option<&mut Context> {
        if let Some(ctx) = self.hot_contexts.get_mut(id) {
            ctx.touch();
            return Some(ctx);
        }

        if let Some(ctx) = self.warm_contexts.get_mut(id) {
            ctx.touch();
            return Some(ctx);
        }

        None
    }

    /// Obtiene múltiples contextos relacionados sin cargar todo
    pub fn get_related(
        &mut self,
        id: &ContextId,
        level: AccessLevel,
    ) -> HashMap<ContextId, PartialContext> {
        let mut result = HashMap::new();

        let context = match self.get(id) {
            Some(c) => c,
            None => return result,
        };

        // Obtener contexto actual (si nivel permite)
        if level >= AccessLevel::Partial {
            result.insert(id.clone(), PartialContext::from_full(context, level));
        }

        // Obtener contextos conectados
        for conn in &context.connections {
            if conn.access_level >= level {
                if let Some(connected) = self.get(&conn.id) {
                    result.insert(
                        conn.id.clone(),
                        PartialContext::from_full(connected, conn.access_level),
                    );
                }
            }
        }

        result
    }

    /// Registra acceso a un contexto
    pub fn touch(&mut self, id: &ContextId) {
        self.working_set.insert(id.clone());
        self.relevance.record_access(id);

        // Mover a hot si está en warm
        if self.warm_contexts.contains_key(id) {
            if let Some(ctx) = self.warm_contexts.remove(id) {
                self.hot_contexts.insert(id.clone(), ctx);
            }
        }

        // Prefetch contextos relacionados
        if self.config.prefetch_enabled {
            self.prefetch_related(id);
        }
    }

    /// Ejecuta prefetch de contextos relacionados
    fn prefetch_related(&mut self, id: &ContextId) {
        let predicted = self.relevance.predict_next(id);

        for pred_id in predicted.into_iter().take(self.config.prefetch_count) {
            if !self.hot_contexts.contains_key(&pred_id)
                && !self.warm_contexts.contains_key(&pred_id)
            {
                self.restore_from_cold(&pred_id);
            }
        }
    }

    /// Busca contextos por query sin cargar todo
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        // Buscar en hot contexts
        for (id, ctx) in &self.hot_contexts {
            let relevance = self.calculate_search_relevance(ctx, &query_lower);
            if relevance > 0.0 {
                results.push(SearchResult {
                    context_id: id.clone(),
                    name: ctx.name.clone(),
                    context_type: ctx.context_type,
                    relevance,
                    state: ContextState::Hot,
                });
            }
        }

        // Buscar en warm contexts
        for (id, ctx) in &self.warm_contexts {
            let relevance = self.calculate_search_relevance(ctx, &query_lower);
            if relevance > 0.0 {
                results.push(SearchResult {
                    context_id: id.clone(),
                    name: ctx.name.clone(),
                    context_type: ctx.context_type,
                    relevance,
                    state: ContextState::Warm,
                });
            }
        }

        // Buscar en cold (índice solamente)
        let cold_results = self.cold_storage.search(query);
        for cold in cold_results {
            results.push(SearchResult {
                context_id: cold.context_id,
                name: cold.name,
                context_type: cold.context_type,
                relevance: cold.relevance_score,
                state: ContextState::Cold,
            });
        }

        // Ordenar por relevancia
        results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());
        results
    }

    fn calculate_search_relevance(&self, ctx: &Context, query: &str) -> f64 {
        let mut score = 0.0;

        // Nombre
        if ctx.name.to_lowercase().contains(query) {
            score += 0.5;
        }

        // Tags
        let tag_matches = ctx
            .tags
            .iter()
            .filter(|t| t.to_lowercase().contains(query))
            .count();
        score += (tag_matches as f64) * 0.2;

        // Summary
        if ctx.summary.to_lowercase().contains(query) {
            score += 0.3;
        }

        score.min(1.0)
    }

    /// Evalúa y ejecuta reciclaje de contextos
    pub fn evaluate_and_recycle(&mut self) -> RecycleStats {
        let mut stats = RecycleStats::default();

        // Recopilar datos para ranking
        let mut all_contexts: Vec<ContextRelevanceData> = Vec::new();

        for (id, ctx) in &self.hot_contexts {
            all_contexts.push(ContextRelevanceData {
                context_id: id.clone(),
                last_access: ctx.metrics.last_access,
                access_count: ctx.metrics.access_count,
                active_connections: ctx.connections.len(),
                priority: ctx.priority,
                connection_to_active: 0, // Simplified
            });
        }

        for (id, ctx) in &self.warm_contexts {
            all_contexts.push(ContextRelevanceData {
                context_id: id.clone(),
                last_access: ctx.metrics.last_access,
                access_count: ctx.metrics.access_count,
                active_connections: ctx.connections.len(),
                priority: ctx.priority,
                connection_to_active: 0,
            });
        }

        // Ranking de relevancia
        let ranked = self.relevance.rank_contexts(&all_contexts);

        // Aplicar acciones basadas en ranking
        for ranked_ctx in ranked {
            if ranked_ctx.should_evict && self.working_set.get(&ranked_ctx.context_id).is_none() {
                // Archivar en frío
                if let Some(ctx) = self
                    .hot_contexts
                    .remove(&ranked_ctx.context_id)
                    .or_else(|| self.warm_contexts.remove(&ranked_ctx.context_id))
                {
                    self.archive_context(ctx);
                    stats.evicted += 1;
                }
            } else if ranked_ctx.should_prefetch {
                stats.prefetched += 1;
            }
        }

        // Mover de hot a warm si hay exceso
        while self.hot_contexts.len() > self.config.max_hot_contexts {
            if let Some((id, _)) = self.hot_contexts.iter().min_by_key(|(id, _)| {
                self.hot_contexts
                    .get(*id)
                    .map(|c| c.metrics.last_access)
                    .unwrap_or(i64::MAX)
            }) {
                let id = id.clone();
                if let Some(ctx) = self.hot_contexts.remove(&id) {
                    self.warm_contexts.insert(id, ctx);
                    stats.demoted += 1;
                }
            } else {
                break;
            }
        }

        stats
    }

    /// Archiva un contexto en frío
    fn archive_context(&mut self, context: Context) {
        let id = context.id.clone();

        // Guardar en cold storage
        if let Err(e) = self.cold_storage.archive(&context) {
            eprintln!("Failed to archive context: {}", e);
            return;
        }

        // Añadir referencia
        self.cold_refs.insert(
            id,
            ColdRef {
                context_id: id.clone(),
                archived_at: now_timestamp(),
                priority: context.priority,
                size_bytes: context.memory_size(),
            },
        );

        self.working_set.remove(&id);
    }

    /// Restaura un contexto desde almacenamiento frío
    fn restore_from_cold(&mut self, id: &ContextId) -> bool {
        // Verificar que está en cold refs
        if !self.cold_refs.contains_key(id) {
            return false;
        }

        // Hacer espacio si es necesario
        if self.hot_contexts.len() >= self.config.max_hot_contexts {
            self.move_hot_to_warm();
        }

        // Restaurar desde cold storage
        match self.cold_storage.full_restore(id) {
            Ok(ctx) => {
                self.warm_contexts.insert(id.clone(), ctx);
                self.cold_refs.remove(id);
                true
            }
            Err(e) => {
                eprintln!("Failed to restore context: {}", e);
                false
            }
        }
    }

    fn move_hot_to_warm(&mut self) {
        if let Some((id, _)) = self
            .hot_contexts
            .iter()
            .filter(|(id, _)| !self.working_set.contains(id))
            .min_by_key(|(id, _)| {
                self.hot_contexts
                    .get(*id)
                    .map(|c| c.metrics.last_access)
                    .unwrap_or(i64::MAX)
            })
        {
            let id = id.clone();
            if let Some(ctx) = self.hot_contexts.remove(&id) {
                self.warm_contexts.insert(id, ctx);
            }
        }
    }

    /// Lista todos los contextos
    pub fn list(&self) -> Vec<ContextInfo> {
        let mut infos = Vec::new();

        for (id, ctx) in &self.hot_contexts {
            infos.push(ContextInfo {
                context_id: id.clone(),
                name: ctx.name.clone(),
                context_type: ctx.context_type,
                state: ContextState::Hot,
                priority: ctx.priority,
                variable_count: ctx.variables.len(),
                connection_count: ctx.connections.len(),
            });
        }

        for (id, ctx) in &self.warm_contexts {
            infos.push(ContextInfo {
                context_id: id.clone(),
                name: ctx.name.clone(),
                context_type: ctx.context_type,
                state: ContextState::Warm,
                priority: ctx.priority,
                variable_count: ctx.variables.len(),
                connection_count: ctx.connections.len(),
            });
        }

        for (id, cold_ref) in &self.cold_refs {
            infos.push(ContextInfo {
                context_id: id.clone(),
                name: format!("Archived-{}", id),
                context_type: ContextType::Session,
                state: ContextState::Cold,
                priority: cold_ref.priority,
                variable_count: 0,
                connection_count: 0,
            });
        }

        infos
    }

    /// Obtiene estadísticas del registry
    pub fn stats(&self) -> RegistryStats {
        let cold_stats = self.cold_storage.stats();

        RegistryStats {
            hot_count: self.hot_contexts.len(),
            warm_count: self.warm_contexts.len(),
            cold_count: self.cold_refs.len(),
            working_set_size: self.working_set.len(),
            max_hot: self.config.max_hot_contexts,
            max_warm: self.config.max_warm_contexts,
            cold_storage_stats: cold_stats,
        }
    }

    /// Obtiene variable del contexto global
    pub fn get_global(&mut self, name: &str) -> Option<&ContextValue> {
        self.global.get(name)
    }

    /// Establece variable global
    pub fn set_global(&mut self, name: &str, value: ContextValue) {
        self.global.set(name, value, VarType::Config);
    }
}

/// Resultado de búsqueda
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub context_id: ContextId,
    pub name: String,
    pub context_type: ContextType,
    pub relevance: f64,
    pub state: ContextState,
}

/// Información de contexto
#[derive(Debug, Clone)]
pub struct ContextInfo {
    pub context_id: ContextId,
    pub name: String,
    pub context_type: ContextType,
    pub state: ContextState,
    pub priority: Priority,
    pub variable_count: usize,
    pub connection_count: usize,
}

/// Contexto parcial (sin cargar todo)
#[derive(Debug, Clone)]
pub struct PartialContext {
    pub id: ContextId,
    pub name: String,
    pub summary: String,
    pub tags: HashSet<String>,
    pub access_level: AccessLevel,
}

impl PartialContext {
    fn from_full(context: &Context, level: AccessLevel) -> Self {
        Self {
            id: context.id.clone(),
            name: context.name.clone(),
            summary: context.summary.clone(),
            tags: context.tags.clone(),
            access_level: level,
        }
    }
}

/// Estadísticas de reciclaje
#[derive(Debug, Default)]
pub struct RecycleStats {
    pub evicted: usize,
    pub demoted: usize,
    pub promoted: usize,
    pub prefetched: usize,
}

/// Estadísticas del registry
#[derive(Debug)]
pub struct RegistryStats {
    pub hot_count: usize,
    pub warm_count: usize,
    pub cold_count: usize,
    pub working_set_size: usize,
    pub max_hot: usize,
    pub max_warm: usize,
    pub cold_storage_stats: ColdStats,
}
