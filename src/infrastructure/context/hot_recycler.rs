//! # Hot Recycler
//!
//! Sistema de reciclaje en CALIENTE para contextos grandes.
//! A diferencia del cold storage, el hot recycler:
//! - Trabaja con contextos en memoria
//! - Divide contextos grandes en chunks
//! - Mantiene el contexto coherente
//! - Recicla partes no usadas frecuentemente

use super::context::Context;
use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Administrador de reciclaje en caliente
pub struct HotRecycler {
    active_chunks: BTreeMap<ChunkId, Chunk>,
    context_chunks: HashMap<ContextId, Vec<ChunkId>>,
    chunk_index: ChunkIndex,
    config: RecyclerConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ChunkId(pub String);

impl ChunkId {
    pub fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(format!("chunk_{:x}", ts))
    }
}

impl Default for ChunkId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: ChunkId,
    pub context_id: ContextId,
    pub chunk_type: ChunkType,
    pub content: String,
    pub keywords: HashSet<String>,
    pub relevance_score: f64,
    pub access_count: u64,
    pub last_access: Timestamp,
    pub size_bytes: usize,
    /// Si está comprimido (para contextos muy grandes)
    pub compressed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkType {
    /// metadata, definitions, setup
    Setup,
    /// main content, core logic
    Core,
    /// Edge cases, variations
    EdgeCases,
    /// Recent additions
    Recent,
    /// Historical, rarely accessed
    History,
    /// Summary of the chunk
    Summary,
}

impl ChunkType {
    pub fn priority(&self) -> u8 {
        match self {
            ChunkType::Core => 0,
            ChunkType::Setup => 1,
            ChunkType::Recent => 2,
            ChunkType::Summary => 3,
            ChunkType::EdgeCases => 4,
            ChunkType::History => 5,
        }
    }
}

/// Índice para búsqueda rápida de chunks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkIndex {
    /// keywords -> chunk ids
    keyword_index: HashMap<String, Vec<ChunkId>>,
    /// Contenido hash -> chunk id (detección de duplicados)
    content_hash: HashMap<String, ChunkId>,
}

impl ChunkIndex {
    pub fn new() -> Self {
        Self {
            keyword_index: HashMap::new(),
            content_hash: HashMap::new(),
        }
    }

    pub fn add(&mut self, chunk: &Chunk) {
        // Indexar por keywords
        for keyword in &chunk.keywords {
            self.keyword_index
                .entry(keyword.clone())
                .or_insert_with(Vec::new)
                .push(chunk.id.clone());
        }

        // Indexar por hash de contenido
        let hash = Self::hash_content(&chunk.content);
        self.content_hash.insert(hash, chunk.id.clone());
    }

    pub fn remove(&mut self, chunk: &Chunk) {
        // Remover de keyword index
        for keyword in &chunk.keywords {
            if let Some(ids) = self.keyword_index.get_mut(keyword) {
                ids.retain(|id| id != &chunk.id);
            }
        }

        // Remover de content hash
        let hash = Self::hash_content(&chunk.content);
        self.content_hash.remove(&hash);
    }

    pub fn search(&self, query: &str) -> Vec<ChunkId> {
        let query_lower = query.to_lowercase();
        let mut results: HashMap<ChunkId, u32> = HashMap::new();

        // Buscar por keywords
        for (keyword, ids) in &self.keyword_index {
            if keyword.to_lowercase().contains(&query_lower) {
                for id in ids {
                    *results.entry(id.clone()).or_insert(0) += 1;
                }
            }
        }

        // Ordenar por frecuencia
        let mut sorted: Vec<_> = results.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        sorted.into_iter().map(|(id, _)| id).collect()
    }

    fn hash_content(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

/// Configuración del recycler
#[derive(Debug, Clone)]
struct RecyclerConfig {
    /// Tamaño máximo de chunk (bytes)
    max_chunk_size: usize,
    /// Máximo de chunks activos
    max_active_chunks: usize,
    /// Umbral para comprimir chunks
    compress_threshold: usize,
    /// Tiempo entre evaluaciones (segundos)
    eval_interval_secs: u64,
}

impl Default for RecyclerConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: 4096, // 4KB
            max_active_chunks: 100,
            compress_threshold: 8192, // 8KB
            eval_interval_secs: 60,
        }
    }
}

impl HotRecycler {
    pub fn new() -> Self {
        Self {
            active_chunks: BTreeMap::new(),
            context_chunks: HashMap::new(),
            chunk_index: ChunkIndex::new(),
            config: RecyclerConfig::default(),
        }
    }

    /// Fragmenta un contexto grande en chunks
    pub fn fragment_context(&mut self, context: &Context) -> Vec<ChunkId> {
        let ctx_id = context.id.clone();
        let mut chunk_ids = Vec::new();

        // Si el contexto es pequeño, crear un solo chunk
        let content = self.serialize_context(context);
        if content.len() <= self.config.max_chunk_size {
            let chunk_id = self.create_chunk(&ctx_id, ChunkType::Core, &content);
            chunk_ids.push(chunk_id);
            return chunk_ids;
        }

        // Fragmentar el contexto
        let chunks = self.smart_fragment(context, &content);

        for (chunk_type, chunk_content) in chunks {
            let chunk_id = self.create_chunk(&ctx_id, chunk_type, &chunk_content);
            chunk_ids.push(chunk_id);
        }

        // Guardar mapeo
        self.context_chunks.insert(ctx_id, chunk_ids.clone());

        chunk_ids
    }

    /// Fragmentación inteligente basada en contenido
    fn smart_fragment(&self, context: &Context, content: &str) -> Vec<(ChunkType, String)> {
        let mut chunks = Vec::new();

        // 1. Setup/metadata
        let metadata = format!(
            "# {}\nType: {:?}\nPriority: {:?}\n",
            context.name, context.context_type, context.priority
        );
        if !metadata.is_empty() {
            chunks.push((ChunkType::Setup, metadata));
        }

        // 2. Variables (si hay)
        if !context.variables.is_empty() {
            let vars_json = serde_json::to_string(&context.variables).unwrap_or_default();
            if vars_json.len() <= self.config.max_chunk_size {
                chunks.push((ChunkType::Core, vars_json));
            } else {
                // Dividir variables
                let var_chunks = self.split_text(&vars_json, self.config.max_chunk_size);
                for (i, vc) in var_chunks.into_iter().enumerate() {
                    let ct = if i == 0 {
                        ChunkType::Core
                    } else {
                        ChunkType::Recent
                    };
                    chunks.push((ct, vc));
                }
            }
        }

        // 3. Resumen
        if !context.summary.is_empty() {
            chunks.push((ChunkType::Summary, context.summary.clone()));
        }

        // 4. Dividir contenido restante en chunks de Core
        let remaining = self.get_remaining_content(context, content);
        if !remaining.is_empty() {
            let core_chunks = self.split_text(&remaining, self.config.max_chunk_size);
            for core_chunk in core_chunks {
                chunks.push((ChunkType::Core, core_chunk));
            }
        }

        // 5. Tags como keywords
        let tags_content = format!("Tags: {:?}", context.tags);
        chunks.push((ChunkType::EdgeCases, tags_content));

        chunks
    }

    /// Divide texto en chunks de tamaño máximo
    fn split_text(&self, text: &str, max_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current = String::new();

        for line in text.lines() {
            if current.len() + line.len() + 1 > max_size {
                if !current.is_empty() {
                    chunks.push(current.clone());
                    current.clear();
                }
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }

    fn serialize_context(&self, context: &Context) -> String {
        serde_json::json!({
            "id": context.id.to_string(),
            "name": context.name,
            "type": context.context_type,
            "variables": context.variables,
            "summary": context.summary,
            "tags": context.tags,
            "connections": context.connections.len(),
        })
        .to_string()
    }

    fn get_remaining_content(&self, context: &Context, _full: &str) -> String {
        // En una implementación real, extraería contenido restante
        // Por ahora retornamos el summary
        context.summary.clone()
    }

    /// Crea un nuevo chunk
    fn create_chunk(
        &mut self,
        ctx_id: &ContextId,
        chunk_type: ChunkType,
        content: &str,
    ) -> ChunkId {
        let id = ChunkId::new();

        let keywords = self.extract_keywords(content);
        let compressed = content.len() > self.config.compress_threshold;

        let chunk = Chunk {
            id: id.clone(),
            context_id: ctx_id.clone(),
            chunk_type,
            content: content.to_string(),
            keywords,
            relevance_score: 0.5,
            access_count: 0,
            last_access: now_timestamp(),
            size_bytes: content.len(),
            compressed,
        };

        self.active_chunks.insert(id.clone(), chunk.clone());
        self.chunk_index.add(&chunk);

        id
    }

    /// Extrae keywords para indexación
    fn extract_keywords(&self, content: &str) -> HashSet<String> {
        let content_lower = content.to_lowercase();
        let words: Vec<&str> = content_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 3)
            .collect();

        let mut freq: HashMap<&str, u32> = HashMap::new();
        for word in &words {
            *freq.entry(word).or_insert(0) += 1;
        }

        freq.into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(word, _)| word.to_string())
            .take(20)
            .collect()
    }

    /// Obtiene chunks relevantes para una query
    pub fn get_relevant_chunks(&self, query: &str) -> Vec<&Chunk> {
        let chunk_ids = self.chunk_index.search(query);

        chunk_ids
            .into_iter()
            .filter_map(|id| self.active_chunks.get(&id))
            .collect()
    }

    /// Obtiene un chunk específico
    pub fn get_chunk(&self, id: &ChunkId) -> Option<&Chunk> {
        self.active_chunks.get(id)
    }

    /// Registra acceso a un chunk y actualiza score
    pub fn touch_chunk(&mut self, id: &ChunkId) {
        if let Some(chunk) = self.active_chunks.get_mut(id) {
            chunk.access_count += 1;
            chunk.last_access = now_timestamp();

            // Actualizar relevancia basada en tipo
            chunk.relevance_score = self.calculate_relevance(chunk);
        }
    }

    fn calculate_relevance(&self, chunk: &Chunk) -> f64 {
        let base = match chunk.chunk_type {
            ChunkType::Core => 1.0,
            ChunkType::Setup => 0.8,
            ChunkType::Recent => 0.7,
            ChunkType::Summary => 0.6,
            ChunkType::EdgeCases => 0.4,
            ChunkType::History => 0.2,
        };

        // Ajustar por frecuencia de acceso
        let access_boost = (chunk.access_count as f64 * 0.01).min(0.3);

        // Ajustar por recencia
        let time_factor = ((now_timestamp() - chunk.last_access) as f64 / 3600.0).min(1.0);
        let recency = 1.0 - time_factor;

        (base + access_boost) * recency
    }

    /// Evalúa y recicla chunks (moviendo poco usados a frío)
    pub fn evaluate_and_recycle(&mut self) -> RecycleResult {
        let mut result = RecycleResult::default();

        // Encontrar chunks con baja relevancia
        let mut to_recycle: Vec<ChunkId> = Vec::new();

        for (id, chunk) in &self.active_chunks {
            // Chunks de History siempre reciclables
            if chunk.chunk_type == ChunkType::History {
                to_recycle.push(id.clone());
            } else if chunk.relevance_score < 0.2 && chunk.access_count < 5 {
                to_recycle.push(id.clone());
            }
        }

        // Mover a reciclaje
        for id in to_recycle.iter().take(10) {
            if let Some(chunk) = self.active_chunks.remove(id) {
                self.chunk_index.remove(&chunk);
                result.recycled += 1;
                result.saved_bytes += chunk.size_bytes;
            }
        }

        // Limpiar chunks huérfanos
        self.context_chunks.retain(|_ctx_id, chunk_ids| {
            chunk_ids.retain(|id| self.active_chunks.contains_key(id));
            !chunk_ids.is_empty()
        });

        result
    }

    /// Reconstruye un contexto desde sus chunks
    pub fn reconstruct_context(&self, ctx_id: &ContextId) -> Option<String> {
        let chunk_ids = self.context_chunks.get(ctx_id)?;

        let mut parts: Vec<String> = Vec::new();
        let mut setup = String::new();
        let mut core = String::new();
        let mut recent = String::new();
        let mut summary = String::new();
        let mut other = String::new();

        for id in chunk_ids {
            if let Some(chunk) = self.active_chunks.get(id) {
                let content = format!(
                    "\n--- {} ---\n{}\n",
                    format!("{:?}", chunk.chunk_type),
                    chunk.content
                );
                match chunk.chunk_type {
                    ChunkType::Setup => setup.push_str(&content),
                    ChunkType::Core => core.push_str(&content),
                    ChunkType::Recent => recent.push_str(&content),
                    ChunkType::Summary => summary.push_str(&content),
                    _ => other.push_str(&content),
                }
            }
        }

        parts.push(setup);
        parts.push(core);
        parts.push(recent);
        parts.push(summary);
        parts.push(other);

        Some(parts.concat())
    }

    /// Lista chunks activos
    pub fn list_chunks(&self) -> Vec<ChunkInfo> {
        self.active_chunks
            .values()
            .map(|c| ChunkInfo {
                id: c.id.clone(),
                context_id: c.context_id.clone(),
                chunk_type: c.chunk_type,
                size_bytes: c.size_bytes,
                relevance_score: c.relevance_score,
                access_count: c.access_count,
            })
            .collect()
    }

    /// Estadísticas del recycler
    pub fn stats(&self) -> RecyclerStats {
        let total_size: usize = self.active_chunks.values().map(|c| c.size_bytes).sum();

        RecyclerStats {
            active_chunks: self.active_chunks.len(),
            total_bytes: total_size,
            contexts_fragmented: self.context_chunks.len(),
            indexed_keywords: self.chunk_index.keyword_index.len(),
        }
    }
}

impl Default for HotRecycler {
    fn default() -> Self {
        Self::new()
    }
}

/// Resultado de evaluación de reciclaje
#[derive(Debug, Default)]
pub struct RecycleResult {
    pub recycled: usize,
    pub saved_bytes: usize,
}

/// Información de chunk
#[derive(Debug, Clone)]
pub struct ChunkInfo {
    pub id: ChunkId,
    pub context_id: ContextId,
    pub chunk_type: ChunkType,
    pub size_bytes: usize,
    pub relevance_score: f64,
    pub access_count: u64,
}

/// Estadísticas del recycler
#[derive(Debug)]
pub struct RecyclerStats {
    pub active_chunks: usize,
    pub total_bytes: usize,
    pub contexts_fragmented: usize,
    pub indexed_keywords: usize,
}
