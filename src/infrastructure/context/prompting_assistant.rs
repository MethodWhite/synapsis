//! # Prompting Assistant
//!
//! Sistema de asistencia inteligente que:
//! - Evalúa constantemente el contexto
//! - Sugiere mejoras y acciones
//! - Detecta qué falta para completar tareas
//! - Actúa proactivamente sin necesidad de instrucciones
//!
//! Este es el "segundo cerebro" que ayuda a la IA principal
//! a tomar mejores decisiones.

use super::context::Context;
use super::hot_recycler::HotRecycler;
use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Motor de asistencia de prompting
pub struct PromptingAssistant {
    /// Modelo de evaluación de contexto
    evaluator: ContextEvaluator,

    /// Generador de sugerencias
    suggester: SuggestionEngine,

    /// Monitor de tareas pendientes
    task_monitor: TaskMonitor,

    /// Historial de decisiones del asistente
    decision_history: Vec<AssistantDecision>,

    /// Configuración
    config: AssistantConfig,
}

/// Configuración del asistente
#[derive(Debug, Clone)]
struct AssistantConfig {
    /// Cuántas sugerencias generar máximo
    max_suggestions: usize,
    /// Umbral de confianza para actuar automáticamente
    auto_action_threshold: f64,
    /// Evaluar cada N segundos
    eval_interval_secs: u64,
    /// Máximo de decisiones en historial
    max_history: usize,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            max_suggestions: 5,
            auto_action_threshold: 0.9,
            eval_interval_secs: 30,
            max_history: 100,
        }
    }
}

/// Evaluador de contexto
#[derive(Debug, Clone)]
struct ContextEvaluator {
    /// Aspectos a evaluar
    aspects: Vec<EvaluationAspect>,
    /// Scores por aspecto
    scores: HashMap<String, f64>,
}

#[derive(Debug, Clone)]
enum EvaluationAspect {
    Completeness,
    Coherence,
    Relevance,
    Freshness,
    Actionability,
}

impl ContextEvaluator {
    fn new() -> Self {
        Self {
            aspects: vec![
                EvaluationAspect::Completeness,
                EvaluationAspect::Coherence,
                EvaluationAspect::Relevance,
                EvaluationAspect::Freshness,
                EvaluationAspect::Actionability,
            ],
            scores: HashMap::new(),
        }
    }

    /// Evalúa un contexto y retorna un reporte
    fn evaluate(&self, context: &Context, _hot_recycler: &HotRecycler) -> EvaluationReport {
        let mut scores = HashMap::new();

        // 1. Completeness: ¿Tiene toda la información necesaria?
        let completeness = self.evaluate_completeness(context);
        scores.insert("completeness".to_string(), completeness);

        // 2. Coherence: ¿El contexto es coherente internamente?
        let coherence = self.evaluate_coherence(context);
        scores.insert("coherence".to_string(), coherence);

        // 3. Relevance: ¿El contexto es relevante para la tarea actual?
        let relevance = self.evaluate_relevance(context);
        scores.insert("relevance".to_string(), relevance);

        // 4. Freshness: ¿Está actualizado?
        let freshness = self.evaluate_freshness(context);
        scores.insert("freshness".to_string(), freshness);

        // 5. Actionability: ¿Se puede actuar con este contexto?
        let actionability = self.evaluate_actionability(context);
        scores.insert("actionability".to_string(), actionability);

        // Calcular score general
        let overall: f64 = scores.values().sum::<f64>() / scores.len() as f64;

        // Identificar gaps
        let gaps = self.identify_gaps(&scores);

        EvaluationReport {
            overall_score: overall,
            aspect_scores: scores,
            gaps,
            recommendations: self.generate_recommendations(&scores),
        }
    }

    fn evaluate_completeness(&self, context: &Context) -> f64 {
        let mut score: f64 = 0.5;

        if context.variables.len() >= 3 {
            score += 0.15;
        }

        if !context.summary.is_empty() {
            score += 0.15;
        }

        if !context.tags.is_empty() {
            score += 0.1;
        }

        if context.connections.len() >= 1 {
            score += 0.1;
        }

        score.min(1.0)
    }

    fn evaluate_coherence(&self, context: &Context) -> f64 {
        let mut score: f64 = 0.7;

        // Verificar que nombre y tipo son consistentes
        if context.name.len() > 0 && !context.name.is_empty() {
            score += 0.1;
        }

        // Verificar que variables tienen tipos consistentes
        // (simplificado - en producción haría análisis más profundo)
        if context.variables.len() < 10 {
            score += 0.2;
        }

        score.min(1.0)
    }

    fn evaluate_relevance(&self, context: &Context) -> f64 {
        let score: f64 = match context.priority {
            Priority::Critical => 1.0,
            Priority::High => 0.8,
            Priority::Normal => 0.6,
            Priority::Low => 0.4,
            Priority::Frozen => 0.2,
        };

        // Ajustar por estado térmico
        score *= match context.state {
            ContextState::Hot => 1.0,
            ContextState::Warm => 0.7,
            ContextState::Cold => 0.3,
        };

        score
    }

    fn evaluate_freshness(&self, context: &Context) -> f64 {
        let age = (now_timestamp() - context.metrics.last_access) as f64;
        let age_hours = age / 3600.0;

        if age_hours < 1.0 {
            1.0
        } else if age_hours < 24.0 {
            0.8
        } else if age_hours < 168.0 {
            // 1 semana
            0.5
        } else {
            0.2
        }
    }

    fn evaluate_actionability(&self, context: &Context) -> f64 {
        let mut score: f64 = 0.4;

        // Tiene metadata accionable
        if context.metadata.len() > 0 {
            score += 0.2;
        }

        // Variables tienen valores concretos
        let has_values = context
            .variables
            .values()
            .any(|v| !matches!(v, super::context::ContextValue::Null));
        if has_values {
            score += 0.2;
        }

        // Puede generar código/output
        if context.context_type == ContextType::Task || context.context_type == ContextType::Project
        {
            score += 0.2;
        }

        score.min(1.0)
    }

    fn identify_gaps(&self, scores: &HashMap<String, f64>) -> Vec<ContextGap> {
        let mut gaps = Vec::new();

        for (aspect, score) in scores {
            if *score < 0.5 {
                gaps.push(ContextGap {
                    aspect: aspect.clone(),
                    severity: if *score < 0.3 {
                        GapSeverity::Critical
                    } else {
                        GapSeverity::Warning
                    },
                    description: self.gap_description(aspect, *score),
                    suggested_action: self.gap_action(aspect),
                });
            }
        }

        gaps
    }

    fn gap_description(&self, aspect: &str, score: f64) -> String {
        match aspect {
            "completeness" => format!("Contexto incompleto (score: {:.0}%)", score * 100.0),
            "coherence" => format!(
                "Contexto podría no ser coherente (score: {:.0}%)",
                score * 100.0
            ),
            "relevance" => format!(
                "Contexto poco relevante para tarea actual (score: {:.0}%)",
                score * 100.0
            ),
            "freshness" => format!("Contexto desactualizado (score: {:.0}%)", score * 100.0),
            "actionability" => format!(
                "Contexto no permite acción directa (score: {:.0}%)",
                score * 100.0
            ),
            _ => format!("Aspecto '{}' necesita atención", aspect),
        }
    }

    fn gap_action(&self, aspect: &str) -> String {
        match aspect {
            "completeness" => "Agregar más variables o información al contexto".to_string(),
            "coherence" => "Revisar que el nombre y tipo sean consistentes".to_string(),
            "relevance" => "Crear un nuevo contexto o actualizar la prioridad".to_string(),
            "freshness" => "Actualizar o refresh el contexto".to_string(),
            "actionability" => "Agregar metadata o definir variables con valores".to_string(),
            _ => "Investigar y corregir".to_string(),
        }
    }

    fn generate_recommendations(&self, scores: &HashMap<String, f64>) -> Vec<String> {
        let mut recs = Vec::new();

        if let Some(&s) = scores.get("completeness") {
            if s < 0.7 {
                recs.push("💡 Considere agregar un resumen o tags al contexto".to_string());
            }
        }

        if let Some(&s) = scores.get("freshness") {
            if s < 0.5 {
                recs.push("⏰ Este contexto no ha sido actualizado recientemente".to_string());
            }
        }

        if let Some(&s) = scores.get("actionability") {
            if s < 0.5 {
                recs.push("🎯 Para actuar, defina variables concretas con valores".to_string());
            }
        }

        recs
    }
}

/// Reporte de evaluación
#[derive(Debug, Clone)]
pub struct EvaluationReport {
    pub overall_score: f64,
    pub aspect_scores: HashMap<String, f64>,
    pub gaps: Vec<ContextGap>,
    pub recommendations: Vec<String>,
}

/// Gap identificado en el contexto
#[derive(Debug, Clone)]
pub struct ContextGap {
    pub aspect: String,
    pub severity: GapSeverity,
    pub description: String,
    pub suggested_action: String,
}

#[derive(Debug, Clone, Copy)]
pub enum GapSeverity {
    Warning,
    Critical,
}

/// Motor de sugerencias
#[derive(Debug, Clone)]
struct SuggestionEngine {
    /// Templates de sugerencias
    templates: Vec<SuggestionTemplate>,
}

#[derive(Debug, Clone)]
struct SuggestionTemplate {
    trigger: String,
    priority: u8,
    suggestion: String,
    action_type: ActionType,
}

#[derive(Debug, Clone, Copy)]
pub enum ActionType {
    AddContext,
    UpdateVariable,
    CreateChunk,
    ArchiveContext,
    MergeContexts,
    RequestUserInput,
    Suggestion,
}

impl SuggestionEngine {
    fn new() -> Self {
        Self {
            templates: vec![
                SuggestionTemplate {
                    trigger: "missing_summary".to_string(),
                    priority: 8,
                    suggestion: "Este contexto no tiene resumen. ¿Desea agregar uno?".to_string(),
                    action_type: ActionType::RequestUserInput,
                },
                SuggestionTemplate {
                    trigger: "large_context".to_string(),
                    priority: 9,
                    suggestion: "Contexto muy grande detectado. Fragmentando en chunks..."
                        .to_string(),
                    action_type: ActionType::CreateChunk,
                },
                SuggestionTemplate {
                    trigger: "stale_context".to_string(),
                    priority: 7,
                    suggestion: "Contexto desactualizado. Sugiriendo archivado...".to_string(),
                    action_type: ActionType::ArchiveContext,
                },
                SuggestionTemplate {
                    trigger: "similar_contexts".to_string(),
                    priority: 6,
                    suggestion: "Se detectaron contextos similares. ¿Desea merge?".to_string(),
                    action_type: ActionType::MergeContexts,
                },
            ],
        }
    }

    /// Genera sugerencias basadas en el estado actual
    fn generate_suggestions(
        &self,
        contexts: &[&Context],
        report: &EvaluationReport,
    ) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        // Basado en gaps de evaluación
        for gap in &report.gaps {
            if let Some(template) = self.templates.iter().find(|t| t.trigger == gap.aspect) {
                suggestions.push(Suggestion {
                    content: template.suggestion.clone(),
                    action_type: template.action_type,
                    priority: template.priority,
                    confidence: 0.8,
                    context_id: None,
                });
            }
        }

        // Basado en patrones de contextos
        let context_count = contexts.len();
        if context_count > 10 {
            suggestions.push(Suggestion {
                content: format!(
                    "{} contextos activos detectados. Considerando archivar inactivos...",
                    context_count
                ),
                action_type: ActionType::ArchiveContext,
                priority: 5,
                confidence: 0.7,
                context_id: None,
            });
        }

        // Ordenar por prioridad
        suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));

        suggestions.truncate(5);
        suggestions
    }
}

/// Sugerencia del asistente
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub content: String,
    pub action_type: ActionType,
    pub priority: u8,
    pub confidence: f64,
    pub context_id: Option<ContextId>,
}

/// Monitor de tareas pendientes
#[derive(Debug, Clone)]
struct TaskMonitor {
    /// Tareas pendientes
    pending_tasks: Vec<PendingTask>,
    /// Tareas completadas recientemente
    completed_tasks: Vec<CompletedTask>,
}

#[derive(Debug, Clone)]
struct PendingTask {
    pub id: String,
    pub description: String,
    pub context_id: Option<ContextId>,
    pub created_at: Timestamp,
    pub priority: Priority,
}

#[derive(Debug, Clone)]
struct CompletedTask {
    pub id: String,
    pub description: String,
    pub completed_at: Timestamp,
}

impl TaskMonitor {
    fn new() -> Self {
        Self {
            pending_tasks: Vec::new(),
            completed_tasks: Vec::new(),
        }
    }

    /// Analiza y sugiere tareas basadas en contexto
    fn suggest_tasks(&self, context: &Context) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Detectar si hay tareas pendientes para este contexto
        let related: Vec<_> = self
            .pending_tasks
            .iter()
            .filter(|t| t.context_id.as_ref() == Some(&context.id))
            .collect();

        if !related.is_empty() {
            suggestions.push(format!(
                "{} tarea(s) pendiente(s) para este contexto",
                related.len()
            ));
        }

        // Sugerir acciones basadas en tipo de contexto
        match context.context_type {
            ContextType::Task if context.state == ContextState::Cold => {
                suggestions.push("Tarea sin actividad reciente. ¿Archivar?".to_string());
            }
            ContextType::Project if context.variables.is_empty() => {
                suggestions
                    .push("Proyecto sin variables definidas. ¿Agregar configuración?".to_string());
            }
            _ => {}
        }

        suggestions
    }

    /// Registra una nueva tarea sugerida
    fn add_pending_task(&mut self, description: String, context_id: Option<ContextId>) {
        self.pending_tasks.push(PendingTask {
            id: format!("task_{}", now_timestamp()),
            description,
            context_id,
            created_at: now_timestamp(),
            priority: Priority::Normal,
        });
    }
}

/// Decisión del asistente
#[derive(Debug, Clone)]
struct AssistantDecision {
    pub timestamp: Timestamp,
    pub decision_type: DecisionType,
    pub context_id: Option<ContextId>,
    pub action_taken: String,
    pub outcome: DecisionOutcome,
}

#[derive(Debug, Clone, Copy)]
enum DecisionType {
    Suggestion,
    AutoAction,
    UserOverride,
}

#[derive(Debug, Clone, Copy)]
enum DecisionOutcome {
    Success,
    Partial,
    Failed,
    Undone,
}

impl PromptingAssistant {
    pub fn new() -> Self {
        Self {
            evaluator: ContextEvaluator::new(),
            suggester: SuggestionEngine::new(),
            task_monitor: TaskMonitor::new(),
            decision_history: Vec::new(),
            config: AssistantConfig::default(),
        }
    }

    /// Evalúa un contexto y genera sugerencias
    pub fn evaluate_and_suggest(
        &mut self,
        context: &Context,
        hot_recycler: &HotRecycler,
    ) -> AssistantOutput {
        // Generar reporte de evaluación
        let report = self.evaluator.evaluate(context, hot_recycler);

        // Generar sugerencias
        let suggestions = self.suggester.generate_suggestions(&[context], &report);

        // Sugerir tareas
        let tasks = self.task_monitor.suggest_tasks(context);

        // Decidir si actuar automáticamente
        let auto_actions = self.decide_auto_actions(&report, &suggestions);

        // Registrar decisión
        self.record_decision(
            DecisionType::Suggestion,
            Some(context.id.clone()),
            format!("Evaluated: {:.0}% complete", report.overall_score * 100.0),
            DecisionOutcome::Success,
        );

        AssistantOutput {
            report,
            suggestions,
            suggested_tasks: tasks,
            auto_actions,
            message: self.generate_message(&report),
        }
    }

    /// Evalúa múltiples contextos
    pub fn evaluate_all(
        &self,
        contexts: &[&Context],
        hot_recycler: &HotRecycler,
    ) -> Vec<(ContextId, EvaluationReport)> {
        contexts
            .iter()
            .map(|ctx| (ctx.id.clone(), self.evaluator.evaluate(ctx, hot_recycler)))
            .collect()
    }

    /// Decide qué acciones tomar automáticamente
    fn decide_auto_actions(
        &self,
        report: &EvaluationReport,
        suggestions: &[Suggestion],
    ) -> Vec<AutoAction> {
        let mut actions = Vec::new();

        // Si el score general es muy bajo, sugerir archive
        if report.overall_score < 0.3 {
            actions.push(AutoAction {
                action_type: ActionType::ArchiveContext,
                reason: "Contexto con muy bajo score".to_string(),
                confidence: 0.6,
            });
        }

        // Si hay gaps críticos, sugerir intervención
        let has_critical = report
            .gaps
            .iter()
            .any(|g| g.severity == GapSeverity::Critical);
        if has_critical {
            actions.push(AutoAction {
                action_type: ActionType::RequestUserInput,
                reason: "Gaps críticos detectados".to_string(),
                confidence: 0.9,
            });
        }

        // Si hay sugerencias de alta prioridad
        for sug in suggestions {
            if sug.confidence >= self.config.auto_action_threshold && sug.priority >= 8 {
                actions.push(AutoAction {
                    action_type: sug.action_type,
                    reason: sug.content.clone(),
                    confidence: sug.confidence,
                });
            }
        }

        actions
    }

    /// Genera mensaje para el usuario/IA
    fn generate_message(&self, report: &EvaluationReport) -> String {
        if report.overall_score >= 0.8 {
            "✅ Contexto en buen estado".to_string()
        } else if report.overall_score >= 0.5 {
            format!(
                "⚠️ Contexto necesita atención: {}",
                report
                    .gaps
                    .first()
                    .map(|g| g.aspect.clone())
                    .unwrap_or_default()
            )
        } else {
            "🔴 Contexto requiere intervención".to_string()
        }
    }

    /// Registra una decisión del asistente
    fn record_decision(
        &mut self,
        decision_type: DecisionType,
        context_id: Option<ContextId>,
        action_taken: String,
        outcome: DecisionOutcome,
    ) {
        self.decision_history.push(AssistantDecision {
            timestamp: now_timestamp(),
            decision_type,
            context_id,
            action_taken,
            outcome,
        });

        // Limitar historial
        if self.decision_history.len() > self.config.max_history {
            self.decision_history.remove(0);
        }
    }

    /// Aprende de las decisiones del usuario
    pub fn learn_from_user(
        &mut self,
        context_id: &ContextId,
        accepted: bool,
        feedback: Option<&str>,
    ) {
        let outcome = if accepted {
            DecisionOutcome::Success
        } else {
            DecisionOutcome::Partial
        };

        self.record_decision(
            DecisionType::UserOverride,
            Some(context_id.clone()),
            feedback
                .map(String::from)
                .unwrap_or_else(|| "User feedback".to_string()),
            outcome,
        );
    }

    /// Genera prompt optimizado para la IA
    pub fn generate_optimized_prompt(&self, context: &Context, query: &str) -> String {
        let mut prompt = String::new();

        // Header
        prompt.push_str("# Contexto Activo\n\n");

        // Nombre y tipo
        prompt.push_str(&format!(
            "**Contexto:** {} ({:?})\n",
            context.name, context.context_type
        ));

        // Resumen si existe
        if !context.summary.is_empty() {
            prompt.push_str(&format!("**Resumen:** {}\n", context.summary));
        }

        // Variables relevantes
        if !context.variables.is_empty() {
            prompt.push_str("\n**Variables:**\n");
            for (key, value) in context.variables.iter().take(5) {
                prompt.push_str(&format!("- {}: {:?}\n", key, value));
            }
        }

        // Query
        prompt.push_str(&format!("\n## Consulta\n\n{}\n", query));

        // Sugerencia del asistente
        prompt.push_str("\n---\n💡 *Sugerencia: ");
        if context.variables.len() < 3 {
            prompt.push_str("Este contexto podría beneficiarse de más variables. ");
        }
        if context.summary.is_empty() {
            prompt.push_str("Considere agregar un resumen para mejor context. ");
        }
        prompt.push_str("*\n");

        prompt
    }

    /// Obtiene historial de decisiones
    pub fn get_decision_history(&self) -> &[AssistantDecision] {
        &self.decision_history
    }

    /// Estadísticas del asistente
    pub fn stats(&self) -> AssistantStats {
        let total_decisions = self.decision_history.len();
        let successful = self
            .decision_history
            .iter()
            .filter(|d| matches!(d.outcome, DecisionOutcome::Success))
            .count();

        AssistantStats {
            total_decisions,
            successful_decisions: successful,
            pending_tasks: self.task_monitor.pending_tasks.len(),
            success_rate: if total_decisions > 0 {
                successful as f64 / total_decisions as f64
            } else {
                0.0
            },
        }
    }
}

impl Default for PromptingAssistant {
    fn default() -> Self {
        Self::new()
    }
}

/// Output del asistente
#[derive(Debug, Clone)]
pub struct AssistantOutput {
    pub report: EvaluationReport,
    pub suggestions: Vec<Suggestion>,
    pub suggested_tasks: Vec<String>,
    pub auto_actions: Vec<AutoAction>,
    pub message: String,
}

/// Acción automática
#[derive(Debug, Clone)]
pub struct AutoAction {
    pub action_type: ActionType,
    pub reason: String,
    pub confidence: f64,
}

/// Estadísticas del asistente
#[derive(Debug)]
pub struct AssistantStats {
    pub total_decisions: usize,
    pub successful_decisions: usize,
    pub pending_tasks: usize,
    pub success_rate: f64,
}
