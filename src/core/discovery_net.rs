//! Synapsis Network Discovery — Cognitive Swarm & P2P Brain Sync.
//! Uses mDNS to find other MethodWhite nodes on the local network.

use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceInfo, ServiceEvent};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct NetworkDiscovery {
    mdns: ServiceDaemon,
    found_nodes: Arc<Mutex<HashMap<String, String>>>, // name -> IP
}

impl NetworkDiscovery {
    pub fn new() -> Result<Self> {
        let mdns = ServiceDaemon::new()?;
        Ok(Self {
            mdns,
            found_nodes: Arc::new(Mutex::new(HashMap::new())),
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
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let node_id = info.get_property_val_str("node_id").unwrap_or("unknown");
                        let ip = info.get_addresses().iter().next().map(|a| a.to_string()).unwrap_or_default();
                        let mut nodes = nodes.lock().unwrap();
                        println!("[Mesh] Discovered Node: {} @ {}", node_id, ip);
                        nodes.insert(node_id.to_string(), ip);
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    pub fn list_nodes(&self) -> HashMap<String, String> {
        self.found_nodes.lock().unwrap().clone()
    }
}
