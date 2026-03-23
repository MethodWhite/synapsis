//! # Global Context
//!
//! Contexto global heredable por todos los contextos.
//! Parámetros del sistema que no deben cargarse completamente
//! pero están disponibles cuando se necesitan.

use super::context::ContextValue;
use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Contexto global del sistema
/// Este contexto NUNCA se carga completamente en memoria
/// Solo se accede a variables específicas bajo demanda
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalContext {
    /// Variables globales (solo se carga bajo demanda)
    variables: HashMap<String, GlobalVar>,

    /// Índices para acceso rápido sin cargar todo
    index: GlobalIndex,

    /// Configuración del sistema
    config: GlobalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlobalVar {
    value: ContextValue,
    access_count: u64,
    last_access: Timestamp,
    /// Si está cacheado, su valor está en memoria
    cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlobalIndex {
    /// Mapeo rápido: nombre de variable -> metadata
    var_metadata: HashMap<String, VarMetadata>,
    /// Variables frecuentemente accedidas juntas
    affinity_groups: HashMap<String, HashSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VarMetadata {
    var_type: VarType,
    size_hint: usize,
    cache_priority: CachePriority,
    /// Desde cuándo está disponible
    available_since: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VarType {
    Config,
    Secret,
    Project,
    Session,
    User,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum CachePriority {
    Always,
    High,
    Normal,
    Lazy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlobalConfig {
    /// Máximo de variables en cache
    max_cached_vars: usize,
    /// Tiempo antes de descachear (segundos)
    cache_ttl: u64,
    /// Variables que siempre están cacheadas
    always_cached: HashSet<String>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            max_cached_vars: 100,
            cache_ttl: 3600, // 1 hora
            always_cached: HashSet::from([
                "system.mode".to_string(),
                "system.version".to_string(),
                "global.locale".to_string(),
            ]),
        }
    }
}

impl GlobalContext {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            index: GlobalIndex::new(),
            config: GlobalConfig::default(),
        }
    }

    /// Obtiene una variable SIN cargar todo el contexto global
    /// Solo carga la variable específica solicitada
    pub fn get(&mut self, name: &str) -> Option<&ContextValue> {
        // Verificar si está cacheado
        if let Some(var) = self.variables.get(name) {
            if var.cached {
                var.access_count += 1;
                var.last_access = now_timestamp();
                return var.value().ok();
            }
        }

        // No está cacheado - cargar bajo demanda
        self.load_var(name)
    }

    /// Carga una variable específica bajo demanda
    fn load_var(&mut self, name: &str) -> Option<&ContextValue> {
        // Verificar si existe en índice
        let metadata = self.index.var_metadata.get(name)?;

        // Decidir si cachear
        let should_cache = self.should_cache(name, metadata);

        if should_cache {
            // Hacer espacio si es necesario
            self.make_cache_space();

            // Cargar variable (en implementación real vendría del almacenamiento)
            let value = self.load_from_storage(name)?;

            let var = GlobalVar {
                value,
                access_count: 1,
                last_access: now_timestamp(),
                cached: true,
            };

            self.variables.insert(name.to_string(), var);
            return self.variables.get(name).and_then(|v| v.value().ok());
        }

        // No cachear, devolver valor directamente
        Some(&ContextValue::String("loaded-not-cached".to_string()))
    }

    fn should_cache(&self, name: &str, metadata: &VarMetadata) -> bool {
        // Siempre cachear si está en la lista
        if self.config.always_cached.contains(name) {
            return true;
        }

        // Usar prioridad de cache
        match metadata.cache_priority {
            CachePriority::Always => true,
            CachePriority::High => self.variables.len() < self.config.max_cached_vars,
            CachePriority::Normal => self.variables.len() < self.config.max_cached_vars / 2,
            CachePriority::Lazy => false,
        }
    }

    fn make_cache_space(&mut self) {
        while self.variables.len() >= self.config.max_cached_vars {
            // Encontrar la variable menos recientemente usada que no sea "always_cached"
            if let Some((name, _)) = self
                .variables
                .iter()
                .filter(|(name, _)| !self.config.always_cached.contains(*name))
                .min_by_key(|(_, var)| var.last_access)
            {
                self.variables.remove(name);
            } else {
                break;
            }
        }
    }

    fn load_from_storage(&self, _name: &str) -> Option<ContextValue> {
        // En implementación real, cargaría del almacenamiento
        // Por ahora retornamos un placeholder
        Some(ContextValue::Null)
    }

    /// Obtiene múltiples variables relacionadas (prefetches)
    pub fn get_related(&mut self, name: &str) -> HashMap<String, ContextValue> {
        let mut result = HashMap::new();

        // Obtener la variable principal
        if let Some(value) = self.get(name) {
            result.insert(name.to_string(), value.clone());
        }

        // Obtener variables relacionadas por afinidad
        if let Some(related) = self.index.affinity_groups.get(name) {
            for rel_name in related {
                if let Some(value) = self.get(rel_name) {
                    result.insert(rel_name.clone(), value.clone());
                }
            }
        }

        result
    }

    /// Registra afinidad entre variables
    pub fn register_affinity(&mut self, var1: &str, var2: &str) {
        self.index
            .affinity_groups
            .entry(var1.to_string())
            .or_insert_with(HashSet::new)
            .insert(var2.to_string());
        self.index
            .affinity_groups
            .entry(var2.to_string())
            .or_insert_with(HashSet::new)
            .insert(var1.to_string());
    }

    /// Establece una variable global
    pub fn set(&mut self, name: &str, value: ContextValue, var_type: VarType) {
        let cached =
            self.config.always_cached.contains(name) || matches!(var_type, VarType::Config);

        let var = GlobalVar {
            value,
            access_count: 0,
            last_access: now_timestamp(),
            cached,
        };

        self.variables.insert(name.to_string(), var);

        // Actualizar índice
        self.index.var_metadata.insert(
            name.to_string(),
            VarMetadata {
                var_type,
                size_hint: 0,
                cache_priority: CachePriority::Normal,
                available_since: now_timestamp(),
            },
        );
    }

    /// Precachea variables comunes juntas
    pub fn prefetch_group(&mut self, names: &[&str]) {
        for name in names {
            let _ = self.get(name);
        }
    }

    /// Serializa SOLO el índice, no los valores
    /// Para reconstruir el global context sin cargar todo
    pub fn serialize_index(&self) -> Vec<u8> {
        serde_json::to_vec(&self.index).unwrap_or_default()
    }

    /// Limpia cache sin afectar almacenamiento
    pub fn clear_cache(&mut self) {
        let always_cached = self.config.always_cached.clone();
        self.variables
            .retain(|name, _| always_cached.contains(name));
    }

    /// Obtiene estadísticas de cache
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            cached_vars: self.variables.len(),
            max_cached: self.config.max_cached_vars,
            total_indexed: self.index.var_metadata.len(),
        }
    }
}

impl Default for GlobalContext {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalIndex {
    fn new() -> Self {
        Self {
            var_metadata: HashMap::new(),
            affinity_groups: HashMap::new(),
        }
    }
}

impl GlobalVar {
    fn value(&self) -> Result<&ContextValue, ()> {
        Ok(&self.value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub cached_vars: usize,
    pub max_cached: usize,
    pub total_indexed: usize,
}
