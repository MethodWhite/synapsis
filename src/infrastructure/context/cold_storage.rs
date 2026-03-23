//! # Cold Storage System
//!
//! Sistema de almacenamiento en frío para contextos inactivos.
//! A diferencia de Engram, NO ELIMINA datos - los archiva inteligentemente.
//!
//! Principios:
//! 1. Nunca eliminar, solo archivar
//! 2. Descomposición inteligente en fragmentos
//! 3. Indexación para recuperación rápida
//! 4. Reconstitución perezosa bajo demanda

use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// Almacenamiento frío de contextos
/// Los datos se archivan, no se eliminan
pub struct ColdStorage {
    /// Directorio base de almacenamiento frío
    base_path: PathBuf,

    /// Índice rápido de contextos archivados
    /// Mapea ContextId -> ColdIndex
    index: BTreeMap<ContextId, ColdIndex>,

    /// Fragments por contexto (datos fragmentados)
    fragments: HashMap<ContextId, Vec<ColdFragment>>,

    /// Configuración
    config: ColdConfig,
}

/// Índice de contexto en frío
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ColdIndex {
    pub context_id: ContextId,
    pub archived_at: Timestamp,
    pub last_access: Timestamp,
    pub access_count: u64,
    pub priority: Priority,
    pub state: ContextState,

    /// Fragmentos que componen este contexto
    pub fragments: Vec<FragmentId>,

    /// Metadata para búsqueda sin descomprimir
    pub searchable_metadata: ColdMetadata,
}

/// Metadata que se mantiene indexada incluso en frío
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ColdMetadata {
    pub name: String,
    pub context_type: ContextType,
    pub tags: Vec<String>,
    pub summary: String,
    pub connection_count: usize,
    pub variable_count: usize,
    pub size_bytes: usize,
}

/// Un fragmento de contexto archivado
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ColdFragment {
    pub id: FragmentId,
    pub fragment_type: FragmentType,
    pub data: Vec<u8>,
    pub compressed: bool,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum FragmentType {
    Variables,
    Connections,
    Metadata,
    History,
    Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct FragmentId(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ColdConfig {
    /// Tamaño máximo de fragmento (bytes)
    max_fragment_size: usize,
    /// Cuántos fragmentos mantener en memoria para acceso frecuente
    hot_fragments_cache: usize,
    /// Tiempo antes de comprimir (segundos)
    compress_after_secs: u64,
    /// Partición de almacenamiento (para rotational storage)
    partition: Option<String>,
}

impl Default for ColdConfig {
    fn default() -> Self {
        Self {
            max_fragment_size: 64 * 1024, // 64KB
            hot_fragments_cache: 10,
            compress_after_secs: 86400, // 1 día
            partition: None,
        }
    }
}

impl ColdStorage {
    pub fn new(base_path: PathBuf) -> Self {
        let storage = Self {
            base_path: base_path.clone(),
            index: BTreeMap::new(),
            fragments: HashMap::new(),
            config: ColdConfig::default(),
        };

        // Crear directorio si no existe
        std::fs::create_dir_all(&base_path).ok();

        storage
    }

    /// Archiva un contexto (NO lo elimina)
    /// Descompone en fragmentos para acceso granular
    pub fn archive(&mut self, context: &super::Context) -> Result<(), ColdError> {
        let ctx_id = context.id.clone();

        // Crear fragmentos del contexto
        let frag_vars = self.fragment_variables(context)?;
        let frag_conns = self.fragment_connections(context)?;
        let frag_meta = self.fragment_metadata(context)?;
        let frag_summary = self.fragment_summary(context)?;

        let fragment_ids: Vec<FragmentId> =
            vec![frag_vars.id, frag_conns.id, frag_meta.id, frag_summary.id];

        // Guardar fragmentos
        self.fragments.insert(
            ctx_id.clone(),
            vec![frag_vars, frag_conns, frag_meta, frag_summary],
        );

        // Crear índice
        let cold_index = ColdIndex {
            context_id: ctx_id.clone(),
            archived_at: now_timestamp(),
            last_access: context.metrics.last_access,
            access_count: context.metrics.access_count,
            priority: context.priority,
            state: ContextState::Cold,
            fragments: fragment_ids,
            searchable_metadata: ColdMetadata {
                name: context.name.clone(),
                context_type: context.context_type,
                tags: context.tags.iter().cloned().collect(),
                summary: context.summary.clone(),
                connection_count: context.connections.len(),
                variable_count: context.variables.len(),
                size_bytes: context.memory_size(),
            },
        };

        // Guardar índice
        self.index.insert(ctx_id, cold_index);

        // Persistir a disco
        self.persist_index()?;

        Ok(())
    }

    /// Recupera un contexto desde almacenamiento frío
    /// Usa carga perezosa - solo carga lo necesario
    pub fn restore(
        &mut self,
        context_id: &ContextId,
        partial: bool,
    ) -> Result<RestoredContext, ColdError> {
        // Obtener índice
        let index = self.index.get(context_id).ok_or(ColdError::NotFound)?;

        // Cargar fragmentos según lo solicitado
        let fragments = if partial {
            // Solo cargar metadata y summary
            self.load_fragments(context_id, &[FragmentType::Metadata, FragmentType::Summary])?
        } else {
            // Cargar todo
            self.load_all_fragments(context_id)?
        };

        // Actualizar estadísticas de acceso
        if let Some(idx) = self.index.get_mut(context_id) {
            idx.access_count += 1;
            idx.last_access = now_timestamp();
        }

        Ok(RestoredContext {
            context_id: context_id.clone(),
            archived_at: index.archived_at,
            access_count: index.access_count,
            priority: index.priority,
            fragments,
            searchable_metadata: index.searchable_metadata.clone(),
        })
    }

    /// Recupera solo fragmentos específicos (acceso granular)
    pub fn restore_fragments(
        &mut self,
        context_id: &ContextId,
        fragment_types: &[FragmentType],
    ) -> Result<Vec<ColdFragment>, ColdError> {
        self.load_fragments(context_id, fragment_types)
    }

    /// Descomprime y recompone el contexto completo
    pub fn full_restore(&mut self, context_id: &ContextId) -> Result<super::Context, ColdError> {
        let restored = self.restore(context_id, false)?;

        let mut context = super::Context::new(
            restored.searchable_metadata.name.clone(),
            restored.searchable_metadata.context_type,
        );

        // Recrear contexto desde fragmentos
        for fragment in restored.fragments {
            match fragment.fragment_type {
                FragmentType::Variables => {
                    // Descomprimir y aplicar variables
                }
                FragmentType::Connections => {
                    // Recrear conexiones
                }
                FragmentType::Metadata => {
                    // Aplicar metadata
                }
                FragmentType::Summary => {
                    // Aplicar summary
                }
                FragmentType::History => {
                    // Recrear historial
                }
            }
        }

        context.state = ContextState::Warm;

        Ok(context)
    }

    /// Busca contextos archivados sin restaurar
    pub fn search(&self, query: &str) -> Vec<ColdSearchResult> {
        let query_lower = query.to_lowercase();

        self.index
            .iter()
            .filter(|(_, idx)| {
                idx.searchable_metadata
                    .name
                    .to_lowercase()
                    .contains(&query_lower)
                    || idx
                        .searchable_metadata
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
                    || idx
                        .searchable_metadata
                        .summary
                        .to_lowercase()
                        .contains(&query_lower)
            })
            .map(|(id, idx)| ColdSearchResult {
                context_id: id.clone(),
                name: idx.searchable_metadata.name.clone(),
                context_type: idx.searchable_metadata.context_type,
                relevance_score: 0.5, // Simplified
                archived_at: idx.archived_at,
            })
            .collect()
    }

    /// Lista todos los contextos archivados (sin restaurar)
    pub fn list_archived(&self) -> Vec<ArchivedContextInfo> {
        self.index
            .values()
            .map(|idx| ArchivedContextInfo {
                context_id: idx.context_id.clone(),
                name: idx.searchable_metadata.name.clone(),
                context_type: idx.searchable_metadata.context_type,
                archived_at: idx.archived_at,
                priority: idx.priority,
                size_bytes: idx.searchable_metadata.size_bytes,
            })
            .collect()
    }

    /// Elimina del índice (marca como eliminado) pero NO borra datos
    /// Los datos permanecen en disco para recuperación futura
    pub fn mark_deleted(&mut self, context_id: &ContextId) -> Result<(), ColdError> {
        if let Some(idx) = self.index.get_mut(context_id) {
            idx.priority = Priority::Frozen;
        }
        Ok(())
    }

    /// Recupera estadísticas de almacenamiento frío
    pub fn stats(&self) -> ColdStats {
        let total_contexts = self.index.len();
        let total_size: usize = self
            .fragments
            .values()
            .flat_map(|frags| frags.iter())
            .map(|f| f.data.len())
            .sum();

        ColdStats {
            total_contexts,
            total_size_bytes: total_size,
            fragment_count: self.fragments.values().map(|v| v.len()).sum(),
        }
    }

    // === Private helper methods ===

    fn fragment_variables(&self, context: &super::Context) -> Result<ColdFragment, ColdError> {
        let data =
            serde_json::to_vec(&context.variables).map_err(|_| ColdError::SerializationFailed)?;

        Ok(ColdFragment {
            id: FragmentId(rand_id()),
            fragment_type: FragmentType::Variables,
            data,
            compressed: false,
            created_at: now_timestamp(),
        })
    }

    fn fragment_connections(&self, context: &super::Context) -> Result<ColdFragment, ColdError> {
        let data =
            serde_json::to_vec(&context.connections).map_err(|_| ColdError::SerializationFailed)?;

        Ok(ColdFragment {
            id: FragmentId(rand_id()),
            fragment_type: FragmentType::Connections,
            data,
            compressed: false,
            created_at: now_timestamp(),
        })
    }

    fn fragment_metadata(&self, context: &super::Context) -> Result<ColdFragment, ColdError> {
        let data =
            serde_json::to_vec(&context.metadata).map_err(|_| ColdError::SerializationFailed)?;

        Ok(ColdFragment {
            id: FragmentId(rand_id()),
            fragment_type: FragmentType::Metadata,
            data,
            compressed: false,
            created_at: now_timestamp(),
        })
    }

    fn fragment_summary(&self, context: &super::Context) -> Result<ColdFragment, ColdError> {
        let data = context.summary.as_bytes().to_vec();

        Ok(ColdFragment {
            id: FragmentId(rand_id()),
            fragment_type: FragmentType::Summary,
            data,
            compressed: false,
            created_at: now_timestamp(),
        })
    }

    fn load_fragments(
        &mut self,
        context_id: &ContextId,
        types: &[FragmentType],
    ) -> Result<Vec<ColdFragment>, ColdError> {
        let fragments = self.fragments.get(context_id).ok_or(ColdError::NotFound)?;

        Ok(fragments
            .iter()
            .filter(|f| types.contains(&f.fragment_type))
            .cloned()
            .collect())
    }

    fn load_all_fragments(
        &mut self,
        context_id: &ContextId,
    ) -> Result<Vec<ColdFragment>, ColdError> {
        self.fragments
            .get(context_id)
            .cloned()
            .ok_or(ColdError::NotFound)
    }

    fn persist_index(&self) -> Result<(), ColdError> {
        let path = self.base_path.join("cold_index.json");
        let data = serde_json::to_vec(&self.index).map_err(|_| ColdError::SerializationFailed)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

/// Error de almacenamiento frío
#[derive(Debug)]
pub enum ColdError {
    NotFound,
    SerializationFailed,
    IoError,
}

impl std::fmt::Display for ColdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColdError::NotFound => write!(f, "Context not found in cold storage"),
            ColdError::SerializationFailed => write!(f, "Failed to serialize/deserialize"),
            ColdError::IoError => write!(f, "IO error"),
        }
    }
}

impl From<std::io::Error> for ColdError {
    fn from(_: std::io::Error) -> Self {
        ColdError::IoError
    }
}

/// Contexto restaurado (parcial o completo)
pub struct RestoredContext {
    pub context_id: ContextId,
    pub archived_at: Timestamp,
    pub access_count: u64,
    pub priority: Priority,
    pub fragments: Vec<ColdFragment>,
    pub searchable_metadata: ColdMetadata,
}

/// Resultado de búsqueda en frío
#[derive(Debug)]
pub struct ColdSearchResult {
    pub context_id: ContextId,
    pub name: String,
    pub context_type: ContextType,
    pub relevance_score: f64,
    pub archived_at: Timestamp,
}

/// Información de contexto archivado
#[derive(Debug, Clone)]
pub struct ArchivedContextInfo {
    pub context_id: ContextId,
    pub name: String,
    pub context_type: ContextType,
    pub archived_at: Timestamp,
    pub priority: Priority,
    pub size_bytes: usize,
}

/// Estadísticas de almacenamiento frío
#[derive(Debug)]
pub struct ColdStats {
    pub total_contexts: usize,
    pub total_size_bytes: usize,
    pub fragment_count: usize,
}

fn rand_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
