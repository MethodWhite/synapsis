//! Synapsis Network Discovery — Cognitive Swarm & P2P Brain Sync.
//! Uses mDNS to find other MethodWhite nodes and MCP servers on the local network.

use crate::core::lock_utils::*;
use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub capabilities: Vec<String>,
    pub protocol: String,
}

pub struct NetworkDiscovery {
    mdns: ServiceDaemon,
    found_nodes: Arc<Mutex<HashMap<String, String>>>,
    discovered_mcp_servers: Arc<Mutex<HashMap<String, McpServerInfo>>>,
}

impl NetworkDiscovery {
    pub fn new() -> Result<Self> {
        let mdns = ServiceDaemon::new()?;
        Ok(Self {
            mdns,
            found_nodes: Arc::new(Mutex::new(HashMap::new())),
            discovered_mcp_servers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Broadcast our presence to the local mesh.
    pub fn broadcast(&self, node_id: &str, port: u16) -> Result<()> {
        let service_type = "_methodwhite._tcp.local.";
        let instance_name = format!("{}.{}", node_id, service_type);

        let mut properties = HashMap::new();
        properties.insert("version".to_string(), "0.1.5".to_string());
        properties.insert("node_id".to_string(), node_id.to_string());

        let service_info = ServiceInfo::new(
            service_type,
            &instance_name,
            &format!("{}.local.", node_id),
            "", // host_ipv4 (auto)
            port,
            Some(properties),
        )?;

        self.mdns.register(service_info)?;
        Ok(())
    }

    /// Scan for other nodes on the network.
    pub fn start_scan(&self) -> Result<()> {
        let receiver = self.mdns.browse("_methodwhite._tcp.local.")?;
        let nodes = self.found_nodes.clone();

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv_async().await {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let node_id = info.get_property_val_str("node_id").unwrap_or("unknown");
                    let ip = info
                        .get_addresses()
                        .iter()
                        .next()
                        .map(|a| a.to_string())
                        .unwrap_or_default();
                    let mut nodes = nodes.lock_safe();
                    println!("[Mesh] Discovered Node: {} @ {}", node_id, ip);
                    nodes.insert(node_id.to_string(), ip);
                }
            }
        });

        Ok(())
    }

    pub fn list_nodes(&self) -> HashMap<String, String> {
        self.found_nodes.lock_safe().clone()
    }

    /// Scan for MCP servers on the network via mDNS (_mcp._tcp.local.).
    pub fn start_mcp_scan(&self) -> Result<()> {
        let receiver = self.mdns.browse("_mcp._tcp.local.")?;
        let servers = self.discovered_mcp_servers.clone();

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv_async().await {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let name = info.get_fullname().to_string();
                    let port = info.get_port();
                    let host = info
                        .get_addresses()
                        .iter()
                        .next()
                        .map(|a| a.to_string())
                        .unwrap_or_default();

                    let capabilities: Vec<String> = info
                        .get_properties()
                        .iter()
                        .filter_map(|prop| {
                            let key = prop.key();
                            if key != "protocol" && key != "version" {
                                Some(format!("{}={}", key, prop.val_str()))
                            } else {
                                None
                            }
                        })
                        .collect();

                    let protocol = info
                        .get_property_val_str("protocol")
                        .unwrap_or("mcp")
                        .to_string();

                    let server_info = McpServerInfo {
                        name: name.clone(),
                        host,
                        port,
                        capabilities,
                        protocol,
                    };

                    let mut servers = servers.lock_safe();
                    println!(
                        "[MCP Mesh] Discovered MCP Server: {} @ {}:{}",
                        name, server_info.host, port
                    );
                    servers.insert(name, server_info);
                }
            }
        });

        Ok(())
    }

    pub fn list_mcp_servers(&self) -> HashMap<String, McpServerInfo> {
        self.discovered_mcp_servers.lock_safe().clone()
    }

    pub fn mcp_server_count(&self) -> usize {
        self.discovered_mcp_servers.lock_safe().len()
    }
}
