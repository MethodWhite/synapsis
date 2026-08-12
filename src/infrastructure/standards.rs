//! Synapsis Standards Module
//!
//! Standards are normative rules ("cómo debe ser X") distinct from skills
//! ("cómo hacer X"). A standard carries a code (e.g. S-46), a normative
//! status, inheritance from parent standards, base references and scope.
//! Compliance is mandatory, whereas skills are optional capabilities.

use crate::core::lock_utils::*;
use crate::core::uuid::Uuid;
use crate::domain::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum StandardStatus {
    Normative = 0,
    Derived = 1,
    Informative = 2,
    Draft = 3,
}

impl std::str::FromStr for StandardStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "normativo" | "normative" | "norma" => Self::Normative,
            "derivado" | "derived" => Self::Derived,
            "informativo" | "informative" | "informativa" => Self::Informative,
            "borrador" | "draft" | "drafting" => Self::Draft,
            _ => Self::Normative,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum StandardCategory {
    Security = 0,
    DevProcess = 1,
    Architecture = 2,
    Compliance = 3,
    Data = 4,
    Testing = 5,
    Documentation = 6,
    ProjectManagement = 7,
    Ops = 8,
    Custom = 9,
}

impl std::str::FromStr for StandardCategory {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "security" | "sec" | "seguridad" => Self::Security,
            "dev-process" | "devprocess" | "devops" | "proceso" | "process" => {
                Self::DevProcess
            }
            "architecture" | "arch" | "arquitectura" => Self::Architecture,
            "compliance" | "grc" | "grc-compliance" => Self::Compliance,
            "data" | "datos" => Self::Data,
            "testing" | "quality" | "calidad" | "pruebas" => Self::Testing,
            "documentation" | "docs" | "documentación" => Self::Documentation,
            "project" | "project-management" | "proyectos" | "pm" => {
                Self::ProjectManagement
            }
            "ops" | "operaciones" | "runbook" => Self::Ops,
            _ => Self::Custom,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct StandardId(pub String);

impl StandardId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_hex_string())
    }
}

impl Default for StandardId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Standard {
    pub id: StandardId,
    pub code: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub category: StandardCategory,
    pub status: StandardStatus,
    pub inherits_from: Vec<String>,
    pub base_refs: Vec<String>,
    pub scope: String,
    pub tags: Vec<String>,
    pub content: String,
    pub enabled: bool,
    pub version: String,
    pub author: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Standard {
    pub fn new(name: String, title: String, description: String) -> Self {
        let now = Timestamp::now();
        Self {
            id: StandardId::new(),
            code: String::new(),
            name,
            title,
            description,
            category: StandardCategory::Custom,
            status: StandardStatus::Normative,
            inherits_from: Vec::new(),
            base_refs: Vec::new(),
            scope: String::new(),
            tags: Vec::new(),
            content: String::new(),
            enabled: true,
            version: "1.0.0".to_string(),
            author: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_code(mut self, code: &str) -> Self {
        self.code = code.to_string();
        self
    }

    pub fn with_category(mut self, category: StandardCategory) -> Self {
        self.category = category;
        self
    }

    pub fn with_status(mut self, status: StandardStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_content(mut self, content: &str) -> Self {
        self.content = content.to_string();
        self
    }

    pub fn with_inherits(mut self, inherits: Vec<String>) -> Self {
        self.inherits_from = inherits;
        self
    }

    pub fn with_bases(mut self, bases: Vec<String>) -> Self {
        self.base_refs = bases;
        self
    }

    pub fn with_scope(mut self, scope: &str) -> Self {
        self.scope = scope.to_string();
        self
    }
}

pub struct StandardRegistry {
    standards: Arc<RwLock<HashMap<StandardId, Standard>>>,
    data_dir: PathBuf,
    dirty: AtomicBool,
    last_save: std::sync::Mutex<Instant>,
}

impl StandardRegistry {
    pub fn new() -> Self {
        let data_dir = crate::config::data_dir().join("standards");
        std::fs::create_dir_all(&data_dir).ok();

        Self {
            standards: Arc::new(RwLock::new(HashMap::new())),
            data_dir,
            dirty: AtomicBool::new(false),
            last_save: std::sync::Mutex::new(Instant::now()),
        }
    }

    pub fn init(&self) -> std::io::Result<()> {
        self.load()?;
        Ok(())
    }

    pub fn load(&self) -> std::io::Result<()> {
        let file = self.data_dir.join("standards.json");
        if file.exists()
            && let Ok(data) = std::fs::read_to_string(&file)
            && let Ok(standards) =
                serde_json::from_str::<HashMap<StandardId, Standard>>(&data)
        {
            *self.standards.write_safe() = standards;
        }
        Ok(())
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        let elapsed = self.last_save.lock_safe().elapsed();
        if elapsed >= std::time::Duration::from_millis(500) {
            let _ = self.flush();
        }
    }

    pub fn flush(&self) -> std::io::Result<()> {
        if self.dirty.swap(false, Ordering::Relaxed) {
            *self.last_save.lock_safe() = Instant::now();
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let file = self.data_dir.join("standards.json");
        let standards = self.standards.read_safe();
        let data = serde_json::to_string_pretty(&*standards)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(file, data)
    }

    pub fn register(&self, standard: Standard) -> StandardId {
        let id = standard.id.clone();
        self.standards.write_safe().insert(id.clone(), standard);
        self.mark_dirty();
        id
    }

    pub fn unregister(&self, id: &StandardId) -> Option<Standard> {
        let standard = self.standards.write_safe().remove(id);
        self.mark_dirty();
        standard
    }

    pub fn get(&self, id: &StandardId) -> Option<Standard> {
        self.standards.read_safe().get(id).cloned()
    }

    pub fn get_by_code(&self, code: &str) -> Option<Standard> {
        self.standards
            .read()
            .unwrap()
            .values()
            .find(|s| !s.code.is_empty() && s.code.eq_ignore_ascii_case(code) && s.enabled)
            .cloned()
    }

    pub fn get_by_name(&self, name: &str) -> Option<Standard> {
        self.standards
            .read()
            .unwrap()
            .values()
            .find(|s| s.name == name && s.enabled)
            .cloned()
    }

    pub fn list(&self) -> Vec<Standard> {
        self.standards
            .read_safe()
            .values()
            .filter(|s| s.enabled)
            .cloned()
            .collect()
    }

    pub fn list_by_category(&self, category: StandardCategory) -> Vec<Standard> {
        self.standards
            .read_safe()
            .values()
            .filter(|s| s.category == category && s.enabled)
            .cloned()
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<Standard> {
        let q = query.to_lowercase();
        self.standards
            .read()
            .unwrap()
            .values()
            .filter(|s| {
                s.enabled
                    && (s.name.to_lowercase().contains(&q)
                        || s.code.to_lowercase().contains(&q)
                        || s.title.to_lowercase().contains(&q)
                        || s.description.to_lowercase().contains(&q)
                        || s.tags.iter().any(|t| t.to_lowercase().contains(&q)))
            })
            .cloned()
            .collect()
    }

    pub fn enable(&self, id: &StandardId) -> bool {
        if let Some(s) = self.standards.write_safe().get_mut(id) {
            s.enabled = true;
            s.updated_at = Timestamp::now();
            self.mark_dirty();
            true
        } else {
            false
        }
    }

    pub fn disable(&self, id: &StandardId) -> bool {
        if let Some(s) = self.standards.write_safe().get_mut(id) {
            s.enabled = false;
            s.updated_at = Timestamp::now();
            self.mark_dirty();
            true
        } else {
            false
        }
    }

    pub fn count(&self) -> usize {
        self.standards.read_safe().len()
    }

    pub fn active_count(&self) -> usize {
        self.standards
            .read()
            .unwrap()
            .values()
            .filter(|s| s.enabled)
            .count()
    }
}

impl Default for StandardRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for StandardRegistry {
    fn clone(&self) -> Self {
        Self {
            standards: self.standards.clone(),
            data_dir: self.data_dir.clone(),
            dirty: AtomicBool::new(false),
            last_save: std::sync::Mutex::new(Instant::now()),
        }
    }
}
