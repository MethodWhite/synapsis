//! Synapsis Agent Classifier
//!
//! Intelligently classifies connecting agents based on:
//! - Connection type (local/remote)
//! - Device recognition
//! - TPM verification
//! - Authentication method
//!
//! # Classification Flow
//!
//! ```text
//! Agent connects
//!     |
//!     v
//! Check connection type (local/remote)
//!     |
//!     v
//! Check device registry (known/unknown)
//!     |
//!     v
//! Check TPM attestation
//!     |
//!     v
//! Apply security rules -> AgentClass
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
// use std::time::{Duration, SystemTime};

use super::permissions::{PermissionSet, TrustLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentClass {
    DeveloperLocal,
    DeveloperRemote,
    TrustedCLI,
    UnknownAgent,
    SuspiciousRemote,
    Blocked,
}

impl fmt::Display for AgentClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentClass::DeveloperLocal => write!(f, "DeveloperLocal"),
            AgentClass::DeveloperRemote => write!(f, "DeveloperRemote"),
            AgentClass::TrustedCLI => write!(f, "TrustedCLI"),
            AgentClass::UnknownAgent => write!(f, "UnknownAgent"),
            AgentClass::SuspiciousRemote => write!(f, "SuspiciousRemote"),
            AgentClass::Blocked => write!(f, "Blocked"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    Local,
    Remote,
    Unknown,
}

impl ConnectionType {
    pub fn from_ip(ip: &IpAddr) -> Self {
        match ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                if octets[0] == 127
                    || (octets[0] == 10 && octets[1] == 0)
                    || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                    || (octets[0] == 192 && octets[1] == 168)
                    || (octets[0] == 0)
                {
                    ConnectionType::Local
                } else {
                    ConnectionType::Remote
                }
            }
            IpAddr::V6(ipv6) => {
                if ipv6.is_loopback() || ipv6.is_unicast_link_local() {
                    ConnectionType::Local
                } else {
                    ConnectionType::Remote
                }
            }
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self, ConnectionType::Local)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub block_unknown_remote: bool,
    pub require_tpm_for_full: bool,
    pub mfa_required_for_new_device: bool,
    pub max_session_duration_hours: u64,
    pub recycle_default_ttl_days: u32,
    pub tpm_required_for_admin: bool,
    pub audit_all_connections: bool,
    pub allowed_cli_types: Vec<String>,
    pub blocked_ip_ranges: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            block_unknown_remote: true,
            require_tpm_for_full: false,
            mfa_required_for_new_device: true,
            max_session_duration_hours: 24,
            recycle_default_ttl_days: 30,
            tpm_required_for_admin: false,
            audit_all_connections: true,
            allowed_cli_types: vec![
                "opencode".into(),
                "qwen".into(),
                "qwen-code".into(),
                "claude".into(),
                "gemini".into(),
                "cursor".into(),
                "windsurf".into(),
                "copilot".into(),
                "synapsis-cli".into(),
            ],
            blocked_ip_ranges: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub device_id: String,
    pub hostname: Option<String>,
    pub ip_addresses: Vec<String>,
    pub tpm_public_key: Option<String>,
    pub tpm_verified: bool,
    pub registered_at: i64,
    pub last_seen: i64,
    pub trust_level: TrustLevel,
    pub owner: String,
    pub device_type: DeviceType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum DeviceType {
    Desktop,
    Laptop,
    Server,
    Mobile,
    IoT,
    #[default]
    Unknown,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub agent_type: String,
    pub client_name: Option<String>,
    pub client_version: Option<String>,
    pub client_type: ClientType,
    pub capabilities: Vec<String>,
    pub has_api_key: bool,
    pub has_dilithium_key: bool,
    pub is_dilithium_verified: bool,
    pub connection_ip: Option<String>,
    pub hostname: Option<String>,
    pub environment: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum ClientType {
    Cli,
    Ide,
    SpecialCLI,
    Browser,
    #[default]
    Unknown,
}


impl ClientType {
    pub fn from_agent_type(agent_type: &str) -> Self {
        match agent_type.to_lowercase().as_str() {
            t if t.contains("cursor") => ClientType::Ide,
            t if t.contains("windsurf") => ClientType::Ide,
            t if t.contains("claude") && !t.contains("code") => ClientType::Cli,
            t if t.contains("copilot") => ClientType::Ide,
            t if t.contains("synapsis") => ClientType::SpecialCLI,
            t if t.contains("opencode") => ClientType::Cli,
            t if t.contains("gemini") => ClientType::Cli,
            t if t.contains("qwen") => ClientType::Cli,
            _ => ClientType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub agent_class: AgentClass,
    pub trust_level: TrustLevel,
    pub permission_set: PermissionSet,
    pub must_encrypt: bool,
    pub can_delegate: bool,
    pub session_timeout: u64,
    pub warnings: Vec<String>,
    pub blocked_reason: Option<String>,
}

impl Default for ClassificationResult {
    fn default() -> Self {
        Self {
            agent_class: AgentClass::Blocked,
            trust_level: TrustLevel::Zero,
            permission_set: PermissionSet::none(),
            must_encrypt: false,
            can_delegate: false,
            session_timeout: 0,
            warnings: vec![],
            blocked_reason: Some("Default blocked".to_string()),
        }
    }
}

pub struct AgentClassifier {
    config: SecurityConfig,
    device_registry: Arc<RwLock<HashMap<String, DeviceRecord>>>,
}

impl AgentClassifier {
    pub fn new() -> Self {
        Self {
            config: SecurityConfig::default(),
            device_registry: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_config(config: SecurityConfig) -> Self {
        Self {
            config,
            device_registry: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn load_registry(&mut self, data_dir: &Path) -> Result<(), std::io::Error> {
        let registry_path = data_dir.join("device_registry.json");
        if registry_path.exists() {
            let data = std::fs::read_to_string(&registry_path)?;
            if let Ok(registry) = serde_json::from_str::<HashMap<String, DeviceRecord>>(&data) {
                let mut reg = self.device_registry.write().unwrap();
                *reg = registry;
            }
        }
        Ok(())
    }

    pub fn save_registry(&self, data_dir: &Path) -> Result<(), std::io::Error> {
        let registry_path = data_dir.join("device_registry.json");
        let reg = self.device_registry.read().unwrap();
        let data = serde_json::to_string_pretty(&*reg)?;
        std::fs::write(registry_path, data)
    }

    pub fn register_device(&self, record: DeviceRecord) {
        let mut reg = self.device_registry.write().unwrap();
        reg.insert(record.device_id.clone(), record);
    }

    pub fn get_device(&self, device_id: &str) -> Option<DeviceRecord> {
        let reg = self.device_registry.read().unwrap();
        reg.get(device_id).cloned()
    }

    pub fn revoke_device(&self, device_id: &str) -> bool {
        let mut reg = self.device_registry.write().unwrap();
        reg.remove(device_id).is_some()
    }

    pub fn set_config(&mut self, config: SecurityConfig) {
        self.config = config;
    }

    pub fn get_config(&self) -> SecurityConfig {
        self.config.clone()
    }

    pub fn classify(
        &self,
        metadata: &AgentMetadata,
        connection_type: ConnectionType,
        device_id: Option<&str>,
        tpm_verified: bool,
    ) -> ClassificationResult {
        let mut warnings = Vec::new();
        let mut blocked_reason = None;

        let is_known_device = device_id.is_some_and(|id| {
            self.device_registry.read().unwrap().contains_key(id)
        });

        let is_local = connection_type.is_local();
        let has_dilithium = metadata.has_dilithium_key && metadata.is_dilithium_verified;
        let is_special_cli = matches!(metadata.client_type, ClientType::SpecialCLI);

        let agent_class = if is_local && is_known_device && tpm_verified {
            AgentClass::DeveloperLocal
        } else if !is_local && is_known_device && tpm_verified {
            AgentClass::DeveloperRemote
        } else if is_local && (has_dilithium || is_special_cli) {
            AgentClass::TrustedCLI
        } else if !is_local && !is_known_device && self.config.block_unknown_remote {
            AgentClass::Blocked
        } else if !is_local && is_known_device && !tpm_verified {
            AgentClass::SuspiciousRemote
        } else if is_local {
            AgentClass::UnknownAgent
        } else if !is_local && !is_known_device {
            AgentClass::SuspiciousRemote
        } else {
            AgentClass::Blocked
        };

        if agent_class == AgentClass::Blocked {
            blocked_reason = Some(
                if !is_local && !is_known_device && self.config.block_unknown_remote {
                    "Remote connection from unknown device blocked by security policy".to_string()
                } else {
                    "Security policy blocked this connection".to_string()
                },
            );
        }

        if is_local && !is_known_device && !has_dilithium {
            warnings
                .push("Local agent without Dilithium key - using basic permissions".to_string());
        }

        if !is_local && is_known_device && !tpm_verified {
            warnings.push(
                "Known remote device without TPM verification - read-only access".to_string(),
            );
        }

        let (trust_level, permission_set) = self.assign_permissions(agent_class, metadata);

        let can_delegate = permission_set.can_delegate;

        let must_encrypt = matches!(
            agent_class,
            AgentClass::DeveloperLocal | AgentClass::DeveloperRemote | AgentClass::TrustedCLI
        ) && !is_local;

        let session_timeout = if metadata.client_type == ClientType::SpecialCLI {
            self.config.max_session_duration_hours * 3600
        } else {
            match agent_class {
                AgentClass::DeveloperLocal | AgentClass::DeveloperRemote => {
                    self.config.max_session_duration_hours * 3600
                }
                AgentClass::TrustedCLI => 43200,
                AgentClass::UnknownAgent => 3600,
                AgentClass::SuspiciousRemote => 1800,
                AgentClass::Blocked => 0,
            }
        };

        ClassificationResult {
            agent_class,
            trust_level,
            permission_set,
            must_encrypt,
            can_delegate,
            session_timeout,
            warnings,
            blocked_reason,
        }
    }

    fn assign_permissions(
        &self,
        class: AgentClass,
        _metadata: &AgentMetadata,
    ) -> (TrustLevel, PermissionSet) {
        match class {
            AgentClass::Blocked => (TrustLevel::Zero, PermissionSet::none()),

            AgentClass::SuspiciousRemote => (
                TrustLevel::Minimal,
                PermissionSet {
                    permissions: PermissionSet::minimal().permissions,
                    max_trust_level: TrustLevel::Minimal,
                    session_timeout: 1800,
                    can_delegate: false,
                },
            ),

            AgentClass::UnknownAgent => (TrustLevel::Basic, PermissionSet::basic()),

            AgentClass::TrustedCLI => (
                TrustLevel::Trusted,
                PermissionSet {
                    permissions: PermissionSet::trusted().permissions,
                    max_trust_level: TrustLevel::Trusted,
                    session_timeout: 43200,
                    can_delegate: true,
                },
            ),

            AgentClass::DeveloperRemote => (
                TrustLevel::Trusted,
                PermissionSet {
                    permissions: PermissionSet::trusted().permissions,
                    max_trust_level: TrustLevel::Trusted,
                    session_timeout: self.config.max_session_duration_hours * 3600,
                    can_delegate: true,
                },
            ),

            AgentClass::DeveloperLocal => (TrustLevel::Firmware, PermissionSet::all()),
        }
    }

    pub fn check_permission(
        &self,
        result: &ClassificationResult,
        permission: super::permissions::Permission,
    ) -> bool {
        result.permission_set.has_permission(permission)
    }

    pub fn should_block(&self, result: &ClassificationResult) -> bool {
        result.agent_class == AgentClass::Blocked
    }

    pub fn requires_mfa(&self, device_id: Option<&str>) -> bool {
        if !self.config.mfa_required_for_new_device {
            return false;
        }

        match device_id {
            Some(id) => {
                let reg = self.device_registry.read().unwrap();
                !reg.contains_key(id)
            }
            None => true,
        }
    }
}

impl Default for AgentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_vs_remote() {
        let local_ip: IpAddr = "127.0.0.1".parse().unwrap();
        let remote_ip: IpAddr = "8.8.8.8".parse().unwrap();

        assert_eq!(ConnectionType::from_ip(&local_ip), ConnectionType::Local);
        assert_eq!(ConnectionType::from_ip(&remote_ip), ConnectionType::Remote);
    }

    #[test]
    fn test_classification_blocked_remote_unknown() {
        let classifier = AgentClassifier::new();

        let metadata = AgentMetadata {
            agent_type: "unknown".to_string(),
            client_name: None,
            client_version: None,
            client_type: ClientType::Unknown,
            capabilities: vec![],
            has_api_key: false,
            has_dilithium_key: false,
            is_dilithium_verified: false,
            connection_ip: Some("8.8.8.8".to_string()),
            hostname: None,
            environment: HashMap::new(),
        };

        let result = classifier.classify(&metadata, ConnectionType::Remote, None, false);

        assert_eq!(result.agent_class, AgentClass::Blocked);
        assert!(result.blocked_reason.is_some());
    }

    #[test]
    fn test_classification_trusted_cli() {
        let classifier = AgentClassifier::new();

        let metadata = AgentMetadata {
            agent_type: "synapsis-cli".to_string(),
            client_name: Some("synapsis".to_string()),
            client_version: Some("1.0.0".to_string()),
            client_type: ClientType::SpecialCLI,
            capabilities: vec!["mcp".to_string()],
            has_api_key: true,
            has_dilithium_key: true,
            is_dilithium_verified: true,
            connection_ip: Some("127.0.0.1".to_string()),
            hostname: Some("localhost".to_string()),
            environment: HashMap::new(),
        };

        let result = classifier.classify(&metadata, ConnectionType::Local, None, false);

        assert_eq!(result.agent_class, AgentClass::TrustedCLI);
        assert!(result
            .permission_set
            .has_permission(super::super::permissions::Permission::PqcEncrypt));
    }

    #[test]
    fn test_classification_local_unknown() {
        let classifier = AgentClassifier::new();

        let metadata = AgentMetadata {
            agent_type: "opencode".to_string(),
            client_name: None,
            client_version: None,
            client_type: ClientType::Cli,
            capabilities: vec![],
            has_api_key: false,
            has_dilithium_key: false,
            is_dilithium_verified: false,
            connection_ip: Some("127.0.0.1".to_string()),
            hostname: None,
            environment: HashMap::new(),
        };

        let result = classifier.classify(&metadata, ConnectionType::Local, None, false);

        assert_eq!(result.agent_class, AgentClass::UnknownAgent);
        assert!(result
            .permission_set
            .has_permission(super::super::permissions::Permission::ReadContext));
    }
}
