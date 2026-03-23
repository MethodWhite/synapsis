//! # Relevance Engine
//!
//! Motor de relevancia para detección inteligente de contexto.
//! A diferencia de Engram, NO CARGA TODO - solo lo relevante.
//!
//! Características:
//! 1. Scoring de relevancia en tiempo real
//! 2. Predicción de contexto futuro
//! 3. Prefetch inteligente basado en patrones
//! 4. Descarte proactivo de contexto irrelevante

use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Motor de relevancia para contextos
pub struct RelevanceEngine {
    /// Modelo de scoring de relevancia
    model: RelevanceModel,

    /// Historial de transiciones para predicción
    transition_graph: TransitionGraph,

    /// Patrones aprendidos de acceso
    learned_patterns: HashMap<String, AccessPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelevanceModel {
    /// Pesos para diferentes features
    weights: RelevanceWeights,

    /// Feature extractors
    features: Vec<FeatureExtractor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelevanceWeights {
    pub recency: f64,
    pub frequency: f64,
    pub affinity: f64,
    pub priority: f64,
    pub connection_strength: f64,
}

impl Default for RelevanceWeights {
    fn default() -> Self {
        Self {
            recency: 0.3,
            frequency: 0.2,
            affinity: 0.2,
            priority: 0.15,
            connection_strength: 0.15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum FeatureExtractor {
    RecentAccess { window_secs: u64 },
    FrequencyCount { window_secs: u64 },
    ConnectionCount,
    PriorityLevel,
    AffinityScore { target_context: String },
    SearchMatch { query: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransitionGraph {
    /// Mapa de transiciones aprendidas: context_id -> (next_context_id, count)
    edges: HashMap<ContextId, HashMap<ContextId, u64>>,

    /// Frecuencia de acceso para predicción
    access_sequence: Vec<ContextId>,

    /// Ventana de secuencia
    sequence_window: usize,
}

impl TransitionGraph {
    fn new() -> Self {
        Self {
            edges: HashMap::new(),
            access_sequence: Vec::new(),
            sequence_window: 100,
        }
    }

    fn record_access(&mut self, context_id: &ContextId) {
        // Agregar a secuencia
        self.access_sequence.push(context_id.clone());
        if self.access_sequence.len() > self.sequence_window {
            self.access_sequence.remove(0);
        }

        // Registrar transición desde el anterior
        if let Some(prev) = self
            .access_sequence
            .get(self.access_sequence.len().saturating_sub(2))
        {
            if prev != context_id {
                self.edges
                    .entry(prev.clone())
                    .or_insert_with(HashMap::new)
                    .entry(context_id.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
            }
        }
    }

    fn predict_next(&self, current: &ContextId) -> Vec<(ContextId, f64)> {
        if let Some(edges) = self.edges.get(current) {
            let total: u64 = edges.values().sum();
            edges
                .iter()
                .map(|(id, count)| (id.clone(), *count as f64 / total as f64))
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessPattern {
    pub trigger: String,
    pub contexts_requested: Vec<ContextId>,
    pub success_count: u64,
    pub failure_count: u64,
}

impl RelevanceEngine {
    pub fn new() -> Self {
        Self {
            model: RelevanceModel {
                weights: RelevanceWeights::default(),
                features: vec![
                    FeatureExtractor::RecentAccess { window_secs: 3600 },
                    FeatureExtractor::FrequencyCount { window_secs: 86400 },
                    FeatureExtractor::ConnectionCount,
                    FeatureExtractor::PriorityLevel,
                ],
            },
            transition_graph: TransitionGraph::new(),
            learned_patterns: HashMap::new(),
        }
    }

    /// Calcula puntuación de relevancia para un contexto
    pub fn score(&self, context: &ContextRelevanceData, weights: &RelevanceWeights) -> f64 {
        let mut score = 0.0;

        // Recencia (0-1, más reciente = mayor)
        let time_since = (now_timestamp() - context.last_access) as f64;
        let recency = (-time_since / 3600.0).exp(); // Decaimiento por hora
        score += weights.recency * recency;

        // Frecuencia (normalizada)
        let frequency = (context.access_count as f64).sqrt() / 10.0;
        score += weights.frequency * frequency.min(1.0);

        // Afinidad (conexiones activas)
        let affinity = (context.active_connections as f64) * 0.1;
        score += weights.affinity * affinity.min(1.0);

        // Prioridad
        let priority_score = match context.priority {
            Priority::Critical => 1.0,
            Priority::High => 0.75,
            Priority::Normal => 0.5,
            Priority::Low => 0.25,
            Priority::Frozen => 0.0,
        };
        score += weights.priority * priority_score;

        // Fuerza de conexión (contexto conectado a uno activo)
        let connection_strength = context.connection_to_active as f64 * 0.2;
        score += weights.connection_strength * connection_strength.min(1.0);

        score
    }

    /// Evalúa relevancia de múltiples contextos y retorna ordenados
    pub fn rank_contexts(&self, contexts: &[ContextRelevanceData]) -> Vec<RankedContext> {
        let weights = &self.model.weights;

        let mut ranked: Vec<RankedContext> = contexts
            .iter()
            .map(|ctx| {
                let score = self.score(ctx, weights);
                RankedContext {
                    context_id: ctx.context_id.clone(),
                    score,
                    should_prefetch: score > 0.7,
                    should_evict: score < 0.2,
                }
            })
            .collect();

        ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        ranked
    }

    /// Predice qué contexto será necesario después del actual
    pub fn predict_next(&self, current: &ContextId) -> Vec<ContextId> {
        self.transition_graph
            .predict_next(current)
            .into_iter()
            .map(|(id, _)| id)
            .take(3)
            .collect()
    }

    /// Registra acceso para aprendizaje de patrones
    pub fn record_access(&mut self, context_id: &ContextId) {
        self.transition_graph.record_access(context_id);
    }

    /// Sugiere prefetch basado en patrones aprendidos
    pub fn suggest_prefetch(&self, current: &ContextId) -> Vec<ContextId> {
        // Basado en predicción de transiciones
        let predicted = self.predict_next(current);

        // Basado en patrones aprendidos
        let mut suggestions = predicted;

        suggestions
    }

    /// Actualiza pesos basado en feedback
    pub fn update_weights(&mut self, feedback: RelevanceFeedback) {
        match feedback {
            RelevanceFeedback::Accessed {
                context_id,
                helpful,
            } => {
                if helpful {
                    self.model.weights.recency *= 1.1;
                } else {
                    self.model.weights.recency *= 0.9;
                }

                // Normalizar
                self.normalize_weights();
            }
            RelevanceFeedback::NotAccessed {
                context_id,
                expected,
            } => {
                if expected {
                    self.model.weights.frequency *= 0.9;
                }
                self.normalize_weights();
            }
        }
    }

    fn normalize_weights(&mut self) {
        let total = self.model.weights.recency
            + self.model.weights.frequency
            + self.model.weights.affinity
            + self.model.weights.priority
            + self.model.weights.connection_strength;

        if total > 0.0 {
            self.model.weights.recency /= total;
            self.model.weights.frequency /= total;
            self.model.weights.affinity /= total;
            self.model.weights.priority /= total;
            self.model.weights.connection_strength /= total;
        }
    }

    /// Detecta qué fragmentos de un contexto son relevantes para una query
    pub fn detect_relevant_fragments(
        &self,
        context: &super::Context,
        query: &str,
    ) -> Vec<FragmentRelevance> {
        let query_lower = query.to_lowercase();
        let mut fragments = Vec::new();

        // Verificar variables
        let var_relevance = context
            .variables
            .iter()
            .filter(|(k, v)| {
                k.to_lowercase().contains(&query_lower)
                    || format!("{:?}", v).to_lowercase().contains(&query_lower)
            })
            .count();

        if var_relevance > 0 {
            fragments.push(FragmentRelevance {
                fragment_type: FragmentType::Variables,
                relevance: (var_relevance as f64 / context.variables.len().max(1) as f64).min(1.0),
            });
        }

        // Verificar tags
        let tag_relevance = context
            .tags
            .iter()
            .filter(|t| t.to_lowercase().contains(&query_lower))
            .count();

        if tag_relevance > 0 {
            fragments.push(FragmentRelevance {
                fragment_type: FragmentType::Tags,
                relevance: (tag_relevance as f64 / context.tags.len().max(1) as f64).min(1.0),
            });
        }

        // Summary siempre es relevante
        if context.summary.to_lowercase().contains(&query_lower) {
            fragments.push(FragmentRelevance {
                fragment_type: FragmentType::Summary,
                relevance: 0.8,
            });
        }

        fragments.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());
        fragments
    }

    /// Aprende un patrón de acceso
    pub fn learn_pattern(&mut self, trigger: &str, contexts: Vec<ContextId>) {
        self.learned_patterns.insert(
            trigger.to_string(),
            AccessPattern {
                trigger: trigger.to_string(),
                contexts_requested: contexts,
                success_count: 1,
                failure_count: 0,
            },
        );
    }
}

/// Datos necesarios para calcular relevancia
#[derive(Debug, Clone)]
pub struct ContextRelevanceData {
    pub context_id: ContextId,
    pub last_access: Timestamp,
    pub access_count: u64,
    pub active_connections: usize,
    pub priority: Priority,
    pub connection_to_active: usize,
}

/// Contexto ranked por relevancia
#[derive(Debug, Clone)]
pub struct RankedContext {
    pub context_id: ContextId,
    pub score: f64,
    pub should_prefetch: bool,
    pub should_evict: bool,
}

/// Relevancia de fragmento
#[derive(Debug, Clone)]
pub struct FragmentRelevance {
    pub fragment_type: FragmentType,
    pub relevance: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum FragmentType {
    Variables,
    Connections,
    Metadata,
    Summary,
    Tags,
}

/// Feedback para ajustar pesos
#[derive(Debug, Clone)]
pub enum RelevanceFeedback {
    Accessed {
        context_id: ContextId,
        helpful: bool,
    },
    NotAccessed {
        context_id: ContextId,
        expected: bool,
    },
}
