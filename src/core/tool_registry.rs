//! Synapsis Tool Registry - Track all discovered tools and capabilities

use crate::core::discovery::{DiscoveredTool, ToolType};
// use crate::domain::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub worker_id: String,
    pub tool_name: String,
    pub tool_type: String,
    pub endpoint: Option<String>,
    pub enabled: bool,
    pub capabilities: Vec<String>,
    pub priority: u8,
}

pub struct ToolRegistry {
    tools: HashMap<String, DiscoveredTool>,
    capability_index: HashMap<String, Vec<String>>,
    enabled_tools: HashMap<String, bool>,
    workers: HashMap<String, WorkerConfig>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            capability_index: HashMap::new(),
            enabled_tools: HashMap::new(),
            workers: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: DiscoveredTool) {
        let name = tool.name.clone();
        self.tools.insert(name.clone(), tool.clone());

        for cap in &tool.capabilities {
            self.capability_index
                .entry(cap.clone())
                .or_default()
                .push(name.clone());
        }
    }

    pub fn unregister(&mut self, tool_name: &str) -> bool {
        if let Some(tool) = self.tools.remove(tool_name) {
            self.enabled_tools.remove(tool_name);
            self.workers.remove(tool_name);

            for cap in &tool.capabilities {
                if let Some(list) = self.capability_index.get_mut(cap) {
                    list.retain(|n| n != tool_name);
                }
            }
            true
        } else {
            false
        }
    }

    pub fn find_by_capability(&self, cap: &str) -> Vec<&DiscoveredTool> {
        self.capability_index
            .get(cap)
            .map(|names| names.iter().filter_map(|n| self.tools.get(n)).collect())
            .unwrap_or_default()
    }

    pub fn find_by_type(&self, tool_type: ToolType) -> Vec<&DiscoveredTool> {
        self.tools
            .values()
            .filter(|t| t.tool_type == tool_type)
            .collect()
    }

    pub fn get_all(&self) -> Vec<&DiscoveredTool> {
        self.tools.values().collect()
    }

    pub fn get_enabled(&self) -> Vec<&DiscoveredTool> {
        self.tools
            .values()
            .filter(|t| self.enabled_tools.get(&t.name).copied().unwrap_or(false))
            .collect()
    }

    pub fn is_enabled(&self, tool_name: &str) -> bool {
        self.enabled_tools.get(tool_name).copied().unwrap_or(false)
    }

    pub fn set_enabled(&mut self, tool_name: &str, enabled: bool) -> bool {
        if self.tools.contains_key(tool_name) {
            self.enabled_tools.insert(tool_name.to_string(), enabled);
            true
        } else {
            false
        }
    }

    pub fn auto_enable(&mut self, tools: Vec<DiscoveredTool>) -> usize {
        let mut count = 0;

        for tool in &tools {
            if tool.auto_integrate && !self.enabled_tools.get(&tool.name).copied().unwrap_or(false)
            {
                self.register(tool.clone());
                self.enabled_tools.insert(tool.name.clone(), true);
                count += 1;
            }
        }

        count
    }

    pub fn get_worker_config(&self, tool: &DiscoveredTool) -> WorkerConfig {
        let worker_id = format!("worker-{}-{}", tool.name, tool.discovered_at);

        let endpoint = tool.path.as_ref().map(|p| p.to_string_lossy().to_string());

        let priority = match tool.tool_type {
            ToolType::AiAgent => 10,
            ToolType::Ide => 8,
            ToolType::DevTool => 5,
            ToolType::PackageManager => 6,
            ToolType::ApiTool => 3,
            ToolType::Linter => 4,
            ToolType::Framework => 7,
            ToolType::Unknown => 1,
        };

        WorkerConfig {
            worker_id,
            tool_name: tool.name.clone(),
            tool_type: tool.tool_type.as_str().to_string(),
            endpoint,
            enabled: self.enabled_tools.get(&tool.name).copied().unwrap_or(false),
            capabilities: tool.capabilities.clone(),
            priority,
        }
    }

    pub fn register_worker(&mut self, config: WorkerConfig) {
        self.workers.insert(config.worker_id.clone(), config);
    }

    pub fn unregister_worker(&mut self, worker_id: &str) -> bool {
        self.workers.remove(worker_id).is_some()
    }

    pub fn get_workers(&self) -> Vec<&WorkerConfig> {
        self.workers.values().collect()
    }

    pub fn get_worker(&self, worker_id: &str) -> Option<&WorkerConfig> {
        self.workers.get(worker_id)
    }

    pub fn find_workers_by_capability(&self, cap: &str) -> Vec<&WorkerConfig> {
        let tool_names: Vec<&str> = self
            .capability_index
            .get(cap)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();

        self.workers
            .values()
            .filter(|w| {
                w.capabilities
                    .iter()
                    .any(|c| tool_names.contains(&c.as_str()))
            })
            .collect()
    }

    pub fn stats(&self) -> serde_json::Value {
        let mut by_type: HashMap<String, usize> = HashMap::new();

        for tool in self.tools.values() {
            *by_type
                .entry(tool.tool_type.as_str().to_string())
                .or_insert(0) += 1;
        }

        serde_json::json!({
            "total_tools": self.tools.len(),
            "enabled_tools": self.enabled_tools.values().filter(|&&v| v).count(),
            "workers": self.workers.len(),
            "by_type": by_type,
            "capabilities": self.capability_index.keys().collect::<Vec<_>>()
        })
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ToolRegistryState(pub Arc<RwLock<ToolRegistry>>);

impl ToolRegistryState {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(ToolRegistry::new())))
    }

    pub fn register(&self, tool: DiscoveredTool) {
        if let Ok(mut registry) = self.0.write() {
            registry.register(tool);
        }
    }

    pub fn find_by_capability(&self, cap: &str) -> Vec<DiscoveredTool> {
        self.0
            .read()
            .ok()
            .map(|r| r.find_by_capability(cap).into_iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get_all(&self) -> Vec<DiscoveredTool> {
        self.0
            .read()
            .ok()
            .map(|r| r.get_all().into_iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn set_enabled(&self, tool_name: &str, enabled: bool) -> bool {
        self.0
            .write()
            .map(|mut r| r.set_enabled(tool_name, enabled))
            .unwrap_or(false)
    }

    pub fn is_enabled(&self, tool_name: &str) -> bool {
        self.0
            .read()
            .map(|r| r.is_enabled(tool_name))
            .unwrap_or(false)
    }

    pub fn stats(&self) -> serde_json::Value {
        self.0
            .read()
            .map(|r| r.stats())
            .unwrap_or(serde_json::json!({"error": "Lock failed"}))
    }
}

impl Default for ToolRegistryState {
    fn default() -> Self {
        Self::new()
    }
}
