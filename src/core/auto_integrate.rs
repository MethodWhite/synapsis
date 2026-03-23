//! Synapsis Auto-Integration - Proactive tool integration

use crate::core::discovery::{
    DiscoveredTool, DiscoveryScan, EnvironmentDiscovery, ToolType,
};
use crate::core::tool_registry::{ToolRegistryState, WorkerConfig};
use crate::domain::types::Timestamp;
use crate::domain::{ErrorKind, Result, SynapsisError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoIntegrateConfig {
    pub scan_interval_secs: u64,
    pub auto_enable_agents: bool,
    pub auto_enable_ides: bool,
    pub auto_enable_package_managers: bool,
    pub emit_events: bool,
}

impl Default for AutoIntegrateConfig {
    fn default() -> Self {
        Self {
            scan_interval_secs: 300,
            auto_enable_agents: true,
            auto_enable_ides: true,
            auto_enable_package_managers: true,
            emit_events: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationEvent {
    pub event_type: String,
    pub tool_name: String,
    pub tool_type: String,
    pub timestamp: i64,
    pub details: serde_json::Value,
}

pub struct AutoIntegrate {
    _discovery: Arc<EnvironmentDiscovery>,
    registry: ToolRegistryState,
    config: AutoIntegrateConfig,
    running: Arc<std::sync::atomic::AtomicBool>,
    event_callback: Option<Box<dyn Fn(IntegrationEvent) + Send + Sync>>,
}

impl AutoIntegrate {
    pub fn new(discovery: Arc<EnvironmentDiscovery>, registry: ToolRegistryState) -> Self {
        Self {
            _discovery: discovery,
            registry,
            config: AutoIntegrateConfig::default(),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_callback: None,
        }
    }

    pub fn with_config(
        discovery: Arc<EnvironmentDiscovery>,
        registry: ToolRegistryState,
        config: AutoIntegrateConfig,
    ) -> Self {
        Self {
            _discovery: discovery,
            registry,
            config,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_callback: None,
        }
    }

    pub fn set_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(IntegrationEvent) + Send + Sync + 'static,
    {
        self.event_callback = Some(Box::new(callback));
    }

    pub fn start_background(&mut self) {
        if self.running.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }

        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let running = Arc::clone(&self.running);

        let discovery = Arc::new(EnvironmentDiscovery::new());
        let registry = ToolRegistryState::new();
        let config = self.config.clone();

        thread::spawn(move || {
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                let result = Self::scan_and_integrate(&discovery, &registry, &config);

                if let Some(event) = result.new_tools.first() {
                    if config.emit_events {
                        println!("[AutoIntegrate] New tool discovered: {}", event.name);
                    }
                }

                thread::sleep(Duration::from_secs(config.scan_interval_secs));
            }
        });
    }

    pub fn stop_background(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn scan_and_integrate(
        discovery: &EnvironmentDiscovery,
        registry: &ToolRegistryState,
        config: &AutoIntegrateConfig,
    ) -> DiscoveryScan {
        let scan = discovery.scan();
        let all_tools = discovery.discover_all();

        for tool in all_tools {
            let should_auto_enable = match tool.tool_type {
                ToolType::AiAgent => config.auto_enable_agents,
                ToolType::Ide => config.auto_enable_ides,
                ToolType::PackageManager => config.auto_enable_package_managers,
                _ => false,
            };

            if should_auto_enable && tool.auto_integrate {
                registry.register(tool.clone());

                let worker_config = registry.0.read().unwrap().get_worker_config(&tool);
                if config.emit_events {
                    Self::emit_event(
                        registry,
                        "tool.auto_integrated",
                        &tool.name,
                        &tool.tool_type.as_str().to_string(),
                        serde_json::json!({"worker_config": worker_config}),
                    );
                }
            }
        }

        scan
    }

    pub fn analyze_capabilities(&self, tool: &DiscoveredTool) -> serde_json::Value {
        let mut analysis = serde_json::json!({
            "name": tool.name,
            "type": tool.tool_type.as_str(),
            "capabilities": tool.capabilities,
            "integration_priority": 0,
            "compatible_with": [],
        });

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

        analysis["integration_priority"] = serde_json::json!(priority);

        let compatible = match tool.tool_type {
            ToolType::AiAgent => vec!["orchestrator", "task_queue", "worker_pool"],
            ToolType::Ide => vec!["editor", "terminal", "debugger"],
            ToolType::DevTool => vec!["shell", "build_system"],
            ToolType::PackageManager => vec!["dependency_resolver", "build_system"],
            ToolType::ApiTool => vec!["http_client", "rest_client"],
            ToolType::Linter => vec!["code_quality", "ci_pipeline"],
            ToolType::Framework => vec!["project_scaffold", "build_system"],
            ToolType::Unknown => vec![],
        };

        analysis["compatible_with"] = serde_json::json!(compatible);

        analysis
    }

    pub fn create_worker_connector(&self, tool: &DiscoveredTool) -> Result<WorkerConfig> {
        let discovery = EnvironmentDiscovery::new();
        let result = discovery.auto_integrate(tool)?;

        if !result.connected {
            return Err(SynapsisError::new(
                ErrorKind::Internal,
                0x0D01,
                &result.error.unwrap_or_default(),
            ));
        }

        let registry = self.registry.0.write().unwrap();
        let config = registry.get_worker_config(tool);

        Ok(config)
    }

    pub fn register_with_task_queue(&self, config: &WorkerConfig) -> Result<()> {
        let mut registry = self.registry.0.write().unwrap();
        registry.register_worker(config.clone());
        Ok(())
    }

    pub fn integrate_with_orchestrator(&self, tool: &DiscoveredTool) -> Result<()> {
        let config = self.create_worker_connector(tool)?;
        self.register_with_task_queue(&config)?;

        Ok(())
    }

    pub fn emit_event(
        _registry: &ToolRegistryState,
        event_type: &str,
        tool_name: &str,
        tool_type: &str,
        details: serde_json::Value,
    ) {
        let _event = IntegrationEvent {
            event_type: event_type.to_string(),
            tool_name: tool_name.to_string(),
            tool_type: tool_type.to_string(),
            timestamp: Timestamp::now().0,
            details,
        };

        println!("[AutoIntegrate] Event: {} - {}", event_type, tool_name);
    }

    pub fn get_status(&self) -> serde_json::Value {
        serde_json::json!({
            "running": self.running.load(std::sync::atomic::Ordering::SeqCst),
            "config": self.config,
            "registry_stats": self.registry.stats(),
        })
    }

    pub fn force_scan(&self) -> DiscoveryScan {
        let discovery = EnvironmentDiscovery::new();
        let scan = discovery.scan();

        for tool in scan.new_tools.clone() {
            self.registry.register(tool);
        }

        scan
    }
}

impl Default for AutoIntegrate {
    fn default() -> Self {
        Self::new(
            Arc::new(EnvironmentDiscovery::new()),
            ToolRegistryState::new(),
        )
    }
}
