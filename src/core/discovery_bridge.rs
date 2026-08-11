//! Synapsis Discovery Bridge — connects EnvironmentDiscovery, NetworkDiscovery,
//! PlatformCatalog, and SessionBridge into a unified discovery pipeline.

use crate::core::discovery::EnvironmentDiscovery;
use crate::core::discovery_net::{McpServerInfo, NetworkDiscovery};
use crate::core::mcp_autoconfig::{AutoConfigReport, detect_and_generate_configs, write_configs};
use crate::core::platform_catalog::detect_installed_platforms;
use crate::core::session_bridge::{SessionBridge, SharedSession, detect_hostname, detect_platform};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryReport {
    pub local_tools: Vec<String>,
    pub mcp_servers: Vec<McpServerInfo>,
    pub network_nodes: Vec<(String, String)>,
    pub auto_configured: Vec<String>,
    pub errors: Vec<String>,
    pub platform_matches: Vec<String>,
}

pub struct DiscoveryBridge {
    env_discovery: EnvironmentDiscovery,
    net_discovery: Option<NetworkDiscovery>,
}

impl DiscoveryBridge {
    pub fn new() -> Result<Self> {
        let net_discovery = match NetworkDiscovery::new() {
            Ok(net) => Some(net),
            Err(e) => {
                eprintln!("[DiscoveryBridge] Network discovery unavailable: {}", e);
                None
            }
        };

        Ok(Self {
            env_discovery: EnvironmentDiscovery::new(),
            net_discovery,
        })
    }

    pub fn with_network(net_discovery: NetworkDiscovery) -> Self {
        Self {
            env_discovery: EnvironmentDiscovery::new(),
            net_discovery: Some(net_discovery),
        }
    }

    pub fn env_discovery(&self) -> &EnvironmentDiscovery {
        &self.env_discovery
    }

    pub fn net_discovery(&self) -> Option<&NetworkDiscovery> {
        self.net_discovery.as_ref()
    }

    /// Full discovery: local PATH + platform catalog + mDNS network + MCP scan.
    pub fn discover_all(&self) -> DiscoveryReport {
        let mut report = DiscoveryReport {
            local_tools: Vec::new(),
            mcp_servers: Vec::new(),
            network_nodes: Vec::new(),
            auto_configured: Vec::new(),
            errors: Vec::new(),
            platform_matches: Vec::new(),
        };

        // 1. Local PATH discovery
        let tools = self.env_discovery.discover_all();
        for tool in &tools {
            report
                .local_tools
                .push(format!("{} ({})", tool.name, tool.tool_type.as_str()));
        }

        // 2. Platform catalog detection
        let installed = detect_installed_platforms();
        for platform in &installed {
            report.platform_matches.push(platform.name.clone());
        }

        // 3. Network discovery (MethodWhite nodes)
        if let Some(ref net) = self.net_discovery {
            let nodes = net.list_nodes();
            for (name, ip) in &nodes {
                report.network_nodes.push((name.clone(), ip.clone()));
            }

            // 4. MCP server discovery
            for (_name, info) in net.list_mcp_servers() {
                report.mcp_servers.push(info);
            }
        }

        report
    }

    /// Auto-configure discovered platforms: generate & write MCP configs.
    pub fn auto_configure(&self, _report: &DiscoveryReport) -> Result<AutoConfigReport> {
        let config_report = detect_and_generate_configs();

        let is_real_binary = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.file_name().map(|n| n.to_string_lossy().to_string()))
            .map(|n| n.starts_with("synapsis"))
            .unwrap_or(false);

        if is_real_binary || std::env::var("SYNAPSIS_AUTOCONFIG_WRITE").is_ok() {
            write_configs(&config_report, false)?;
        } else {
            eprintln!(
                "[DiscoveryBridge] Skipping config writes: process is not the synapsis binary"
            );
        }

        Ok(config_report)
    }

    /// Register discovered AI agents as shared sessions in the bridge.
    pub fn register_discovered_agents(&self, report: &DiscoveryReport) {
        let bridge = SessionBridge::global();
        let hostname = detect_hostname();
        let platform = detect_platform();

        for tool_name in &report.local_tools {
            let name_clean = tool_name.split(" (").next().unwrap_or(tool_name);
            let session = SharedSession::new(
                &format!(
                    "discovery-{}-{}",
                    name_clean,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                ),
                name_clean,
                "ai_agent",
                &hostname,
                "auto-discovered",
                &platform,
            );
            bridge.register_session(session);
        }

        for server in &report.mcp_servers {
            let session = SharedSession::new(
                &format!(
                    "mcp-{}-{}",
                    server.name,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                ),
                &server.name,
                "mcp_server",
                &server.host,
                "auto-discovered",
                &platform,
            );
            bridge.register_session(session);
        }
    }

    /// Convenience: discover + configure + register in one call.
    pub fn full_discovery_flow(&self) -> DiscoveryReport {
        let report = self.discover_all();

        if let Ok(config_report) = self.auto_configure(&report) {
            for entry in &config_report.generated {
                println!(
                    "[DiscoveryBridge] Auto-configured {} -> {}",
                    entry.platform_name, entry.config_path
                );
            }
        }

        self.register_discovered_agents(&report);
        report
    }

    /// Get summary stats for the report.
    pub fn report_summary(report: &DiscoveryReport) -> HashMap<String, usize> {
        let mut summary = HashMap::new();
        summary.insert("local_tools".to_string(), report.local_tools.len());
        summary.insert("mcp_servers".to_string(), report.mcp_servers.len());
        summary.insert("network_nodes".to_string(), report.network_nodes.len());
        summary.insert("auto_configured".to_string(), report.auto_configured.len());
        summary.insert(
            "platform_matches".to_string(),
            report.platform_matches.len(),
        );
        summary.insert("errors".to_string(), report.errors.len());
        summary
    }
}
