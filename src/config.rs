//! Synapsis Configuration
//!
//! TOML configuration file parsing y gestión.

use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;

/// Default config values
pub const DEFAULT_PORT: u16 = 7438;
pub const DEFAULT_DATA_DIR: &str = ".synapsis";
pub const DEFAULT_MAX_OBS_LENGTH: usize = 100_000;
pub const DEFAULT_MAX_CONTEXT_RESULTS: usize = 20;
pub const DEFAULT_MAX_SEARCH_RESULTS: usize = 20;
pub const DEFAULT_DEDUPE_WINDOW_SECS: i64 = 900; // 15 minutes

/// Configuration schema
#[derive(Debug, Clone)]
pub struct Config {
    /// General settings
    pub general: GeneralConfig,
    /// Server settings
    pub server: ServerConfig,
    /// Storage settings
    pub storage: StorageConfig,
    /// Security settings
    pub security: SecurityConfig,
    /// MCP settings
    pub mcp: McpConfig,
    /// Sync settings
    pub sync: SyncConfig,
    /// Logging settings
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone)]
pub struct GeneralConfig {
    pub data_dir: String,
    pub profile: String,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: u32,
    pub tls_enabled: bool,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub max_observation_length: usize,
    pub max_context_results: usize,
    pub max_search_results: usize,
    pub dedupe_window_secs: i64,
    pub vacuum_on_startup: bool,
    pub checkpoint_interval_secs: i64,
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub pqc_enabled: bool,
    pub encryption_enabled: bool,
    pub integrity_check_enabled: bool,
    pub integrity_check_interval_secs: i64,
    pub max_failed_auths: u32,
    pub session_timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub struct McpConfig {
    pub tools: Vec<String>,
    pub profile: String,
    pub auto_session: bool,
    pub auto_import: bool,
}

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub enabled: bool,
    pub remote_url: Option<String>,
    pub sync_interval_secs: i64,
    pub conflict_resolution: ConflictResolution,
    pub enrolled_projects: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ConflictResolution {
    LocalWins,
    RemoteWins,
    NewestWins,
    Manual,
}

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub output: String,
    pub rotation: LogRotation,
}

#[derive(Debug, Clone)]
pub struct LogRotation {
    pub max_size_mb: u64,
    pub max_files: u32,
    pub compress: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            server: ServerConfig::default(),
            storage: StorageConfig::default(),
            security: SecurityConfig::default(),
            mcp: McpConfig::default(),
            sync: SyncConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            profile: "default".to_string(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: DEFAULT_PORT,
            workers: 1,
            tls_enabled: false,
            tls_cert: None,
            tls_key: None,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            max_observation_length: DEFAULT_MAX_OBS_LENGTH,
            max_context_results: DEFAULT_MAX_CONTEXT_RESULTS,
            max_search_results: DEFAULT_MAX_SEARCH_RESULTS,
            dedupe_window_secs: DEFAULT_DEDUPE_WINDOW_SECS,
            vacuum_on_startup: false,
            checkpoint_interval_secs: 3600, // 1 hour
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            pqc_enabled: true,
            encryption_enabled: true,
            integrity_check_enabled: true,
            integrity_check_interval_secs: 300, // 5 minutes
            max_failed_auths: 5,
            session_timeout_secs: 86400, // 24 hours
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            tools: vec![],
            profile: "all".to_string(),
            auto_session: true,
            auto_import: true,
        }
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            remote_url: None,
            sync_interval_secs: 60,
            conflict_resolution: ConflictResolution::NewestWins,
            enrolled_projects: vec![],
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
            output: "stdout".to_string(),
            rotation: LogRotation::default(),
        }
    }
}

impl Default for LogRotation {
    fn default() -> Self {
        Self {
            max_size_mb: 10,
            max_files: 5,
            compress: true,
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let content = read_file(path)?;
        Self::parse(&content)
    }

    /// Load from default locations
    pub fn load_default() -> Result<Self, ConfigError> {
        // Try XDG_CONFIG_HOME first
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            let path = format!("{}/synapsis/config.toml", xdg);
            if file_exists(&path) {
                return Self::load(&path);
            }
        }

        // Try ~/.config/synapsis/config.toml
        if let Ok(home) = std::env::var("HOME") {
            let path = format!("{}/.config/synapsis/config.toml", home);
            if file_exists(&path) {
                return Self::load(&path);
            }
        }

        // Try ~/.synapsis/config.toml
        if let Ok(home) = std::env::var("HOME") {
            let path = format!("{}/.synapsis/config.toml", home);
            if file_exists(&path) {
                return Self::load(&path);
            }
        }

        // Return default config
        Ok(Self::default())
    }

    /// Parse TOML content
    pub fn parse(content: &str) -> Result<Self, ConfigError> {
        let mut config = Self::default();
        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Section header
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                continue;
            }

            // Key-value pair
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();

                match current_section.as_str() {
                    "" | "general" => Self::parse_general(&mut config.general, key, value),
                    "server" => Self::parse_server(&mut config.server, key, value),
                    "storage" => Self::parse_storage(&mut config.storage, key, value),
                    "security" => Self::parse_security(&mut config.security, key, value),
                    "mcp" => Self::parse_mcp(&mut config.mcp, key, value),
                    "sync" => Self::parse_sync(&mut config.sync, key, value),
                    "logging" => Self::parse_logging(&mut config.logging, key, value),
                    _ => {} // Unknown section, skip
                }
            }
        }

        Ok(config)
    }

    fn parse_general(cfg: &mut GeneralConfig, key: &str, value: &str) {
        match key {
            "data_dir" => cfg.data_dir = value.to_string(),
            "profile" => cfg.profile = value.to_string(),
            _ => {}
        }
    }

    fn parse_server(cfg: &mut ServerConfig, key: &str, value: &str) {
        match key {
            "host" => cfg.host = value.to_string(),
            "port" => cfg.port = value.parse().unwrap_or(DEFAULT_PORT),
            "workers" => cfg.workers = value.parse().unwrap_or(1),
            "tls_enabled" => cfg.tls_enabled = value.parse().unwrap_or(false),
            "tls_cert" => cfg.tls_cert = Some(value.to_string()),
            "tls_key" => cfg.tls_key = Some(value.to_string()),
            _ => {}
        }
    }

    fn parse_storage(cfg: &mut StorageConfig, key: &str, value: &str) {
        match key {
            "max_observation_length" => {
                cfg.max_observation_length = value.parse().unwrap_or(DEFAULT_MAX_OBS_LENGTH)
            }
            "max_context_results" => {
                cfg.max_context_results = value.parse().unwrap_or(DEFAULT_MAX_CONTEXT_RESULTS)
            }
            "max_search_results" => {
                cfg.max_search_results = value.parse().unwrap_or(DEFAULT_MAX_SEARCH_RESULTS)
            }
            "dedupe_window_secs" => {
                cfg.dedupe_window_secs = value.parse().unwrap_or(DEFAULT_DEDUPE_WINDOW_SECS)
            }
            "vacuum_on_startup" => cfg.vacuum_on_startup = value.parse().unwrap_or(false),
            "checkpoint_interval_secs" => {
                cfg.checkpoint_interval_secs = value.parse().unwrap_or(3600)
            }
            _ => {}
        }
    }

    fn parse_security(cfg: &mut SecurityConfig, key: &str, value: &str) {
        match key {
            "pqc_enabled" => cfg.pqc_enabled = value.parse().unwrap_or(true),
            "encryption_enabled" => cfg.encryption_enabled = value.parse().unwrap_or(true),
            "integrity_check_enabled" => {
                cfg.integrity_check_enabled = value.parse().unwrap_or(true)
            }
            "integrity_check_interval_secs" => {
                cfg.integrity_check_interval_secs = value.parse().unwrap_or(300)
            }
            "max_failed_auths" => cfg.max_failed_auths = value.parse().unwrap_or(5),
            "session_timeout_secs" => cfg.session_timeout_secs = value.parse().unwrap_or(86400),
            _ => {}
        }
    }

    fn parse_mcp(cfg: &mut McpConfig, key: &str, value: &str) {
        match key {
            "tools" => cfg.tools = value.split(',').map(|s| s.trim().to_string()).collect(),
            "profile" => cfg.profile = value.to_string(),
            "auto_session" => cfg.auto_session = value.parse().unwrap_or(true),
            "auto_import" => cfg.auto_import = value.parse().unwrap_or(true),
            _ => {}
        }
    }

    fn parse_sync(cfg: &mut SyncConfig, key: &str, value: &str) {
        match key {
            "enabled" => cfg.enabled = value.parse().unwrap_or(false),
            "remote_url" => cfg.remote_url = Some(value.to_string()),
            "sync_interval_secs" => cfg.sync_interval_secs = value.parse().unwrap_or(60),
            "conflict_resolution" => {
                cfg.conflict_resolution = match value {
                    "local_wins" => ConflictResolution::LocalWins,
                    "remote_wins" => ConflictResolution::RemoteWins,
                    "newest_wins" => ConflictResolution::NewestWins,
                    "manual" => ConflictResolution::Manual,
                    _ => ConflictResolution::NewestWins,
                }
            }
            "enrolled_projects" => {
                cfg.enrolled_projects = value.split(',').map(|s| s.trim().to_string()).collect()
            }
            _ => {}
        }
    }

    fn parse_logging(cfg: &mut LoggingConfig, key: &str, value: &str) {
        match key {
            "level" => cfg.level = value.to_string(),
            "format" => cfg.format = value.to_string(),
            "output" => cfg.output = value.to_string(),
            "max_size_mb" => cfg.rotation.max_size_mb = value.parse().unwrap_or(10),
            "max_files" => cfg.rotation.max_files = value.parse().unwrap_or(5),
            "compress" => cfg.rotation.compress = value.parse().unwrap_or(true),
            _ => {}
        }
    }

    /// Save configuration to file
    pub fn save(&self, path: &str) -> Result<(), ConfigError> {
        let content = self.to_toml();
        write_file(path, &content)
    }

    /// Convert to TOML string
    pub fn to_toml(&self) -> String {
        let mut s = String::new();

        s.push_str("# Synapsis Configuration\n\n");

        s.push_str("[general]\n");
        s.push_str(&format!("data_dir = \"{}\"\n", self.general.data_dir));
        s.push_str(&format!("profile = \"{}\"\n\n", self.general.profile));

        s.push_str("[server]\n");
        s.push_str(&format!("host = \"{}\"\n", self.server.host));
        s.push_str(&format!("port = {}\n", self.server.port));
        s.push_str(&format!("workers = {}\n", self.server.workers));
        s.push_str(&format!("tls_enabled = {}\n", self.server.tls_enabled));
        if let Some(ref cert) = self.server.tls_cert {
            s.push_str(&format!("tls_cert = \"{}\"\n", cert));
        }
        if let Some(ref key) = self.server.tls_key {
            s.push_str(&format!("tls_key = \"{}\"\n", key));
        }
        s.push('\n');

        s.push_str("[storage]\n");
        s.push_str(&format!(
            "max_observation_length = {}\n",
            self.storage.max_observation_length
        ));
        s.push_str(&format!(
            "max_context_results = {}\n",
            self.storage.max_context_results
        ));
        s.push_str(&format!(
            "max_search_results = {}\n",
            self.storage.max_search_results
        ));
        s.push_str(&format!(
            "dedupe_window_secs = {}\n",
            self.storage.dedupe_window_secs
        ));
        s.push_str(&format!(
            "vacuum_on_startup = {}\n",
            self.storage.vacuum_on_startup
        ));
        s.push_str(&format!(
            "checkpoint_interval_secs = {}\n",
            self.storage.checkpoint_interval_secs
        ));
        s.push('\n');

        s.push_str("[security]\n");
        s.push_str(&format!("pqc_enabled = {}\n", self.security.pqc_enabled));
        s.push_str(&format!(
            "encryption_enabled = {}\n",
            self.security.encryption_enabled
        ));
        s.push_str(&format!(
            "integrity_check_enabled = {}\n",
            self.security.integrity_check_enabled
        ));
        s.push_str(&format!(
            "integrity_check_interval_secs = {}\n",
            self.security.integrity_check_interval_secs
        ));
        s.push_str(&format!(
            "max_failed_auths = {}\n",
            self.security.max_failed_auths
        ));
        s.push_str(&format!(
            "session_timeout_secs = {}\n",
            self.security.session_timeout_secs
        ));
        s.push('\n');

        s.push_str("[mcp]\n");
        if !self.mcp.tools.is_empty() {
            s.push_str(&format!("tools = \"{}\"\n", self.mcp.tools.join(", ")));
        }
        s.push_str(&format!("profile = \"{}\"\n", self.mcp.profile));
        s.push_str(&format!("auto_session = {}\n", self.mcp.auto_session));
        s.push_str(&format!("auto_import = {}\n", self.mcp.auto_import));
        s.push('\n');

        s.push_str("[sync]\n");
        s.push_str(&format!("enabled = {}\n", self.sync.enabled));
        if let Some(ref url) = self.sync.remote_url {
            s.push_str(&format!("remote_url = \"{}\"\n", url));
        }
        s.push_str(&format!(
            "sync_interval_secs = {}\n",
            self.sync.sync_interval_secs
        ));
        let conflict = match self.sync.conflict_resolution {
            ConflictResolution::LocalWins => "local_wins",
            ConflictResolution::RemoteWins => "remote_wins",
            ConflictResolution::NewestWins => "newest_wins",
            ConflictResolution::Manual => "manual",
        };
        s.push_str(&format!("conflict_resolution = \"{}\"\n", conflict));
        if !self.sync.enrolled_projects.is_empty() {
            s.push_str(&format!(
                "enrolled_projects = \"{}\"\n",
                self.sync.enrolled_projects.join(", ")
            ));
        }
        s.push('\n');

        s.push_str("[logging]\n");
        s.push_str(&format!("level = \"{}\"\n", self.logging.level));
        s.push_str(&format!("format = \"{}\"\n", self.logging.format));
        s.push_str(&format!("output = \"{}\"\n", self.logging.output));
        s.push_str(&format!(
            "max_size_mb = {}\n",
            self.logging.rotation.max_size_mb
        ));
        s.push_str(&format!(
            "max_files = {}\n",
            self.logging.rotation.max_files
        ));
        s.push_str(&format!("compress = {}\n", self.logging.rotation.compress));

        s
    }

    /// Get value by key path (e.g., "server.port")
    pub fn get(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split('.').collect();

        match parts.as_slice() {
            ["general", k] => self.general.get(k),
            ["server", k] => self.server.get(k),
            ["storage", k] => self.storage.get(k),
            ["security", k] => self.security.get(k),
            ["mcp", k] => self.mcp.get(k),
            ["sync", k] => self.sync.get(k),
            ["logging", k] => self.logging.get(k),
            _ => None,
        }
    }

    /// Set value by key path
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        let parts: Vec<&str> = key.split('.').collect();

        match parts.as_slice() {
            ["general", k] => self.general.set(k, value),
            ["server", k] => self.server.set(k, value),
            ["storage", k] => self.storage.set(k, value),
            ["security", k] => self.security.set(k, value),
            ["mcp", k] => self.mcp.set(k, value),
            ["sync", k] => self.sync.set(k, value),
            ["logging", k] => self.logging.set(k, value),
            _ => Err(ConfigError::InvalidKey(key.to_string())),
        }
    }

    /// Initialize default config file
    pub fn init(path: &str) -> Result<(), ConfigError> {
        let config = Self::default();
        config.save(path)
    }
}

impl GeneralConfig {
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "data_dir" => Some(self.data_dir.clone()),
            "profile" => Some(self.profile.clone()),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        match key {
            "data_dir" => {
                self.data_dir = value.to_string();
                Ok(())
            }
            "profile" => {
                self.profile = value.to_string();
                Ok(())
            }
            _ => Err(ConfigError::InvalidKey(format!("general.{}", key))),
        }
    }
}

impl ServerConfig {
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "host" => Some(self.host.clone()),
            "port" => Some(self.port.to_string()),
            "workers" => Some(self.workers.to_string()),
            "tls_enabled" => Some(self.tls_enabled.to_string()),
            "tls_cert" => self.tls_cert.clone(),
            "tls_key" => self.tls_key.clone(),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        match key {
            "host" => {
                self.host = value.to_string();
                Ok(())
            }
            "port" => {
                self.port = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "workers" => {
                self.workers = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "tls_enabled" => {
                self.tls_enabled = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "tls_cert" => {
                self.tls_cert = Some(value.to_string());
                Ok(())
            }
            "tls_key" => {
                self.tls_key = Some(value.to_string());
                Ok(())
            }
            _ => Err(ConfigError::InvalidKey(format!("server.{}", key))),
        }
    }
}

impl StorageConfig {
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "max_observation_length" => Some(self.max_observation_length.to_string()),
            "max_context_results" => Some(self.max_context_results.to_string()),
            "max_search_results" => Some(self.max_search_results.to_string()),
            "dedupe_window_secs" => Some(self.dedupe_window_secs.to_string()),
            "vacuum_on_startup" => Some(self.vacuum_on_startup.to_string()),
            "checkpoint_interval_secs" => Some(self.checkpoint_interval_secs.to_string()),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        match key {
            "max_observation_length" => {
                self.max_observation_length = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "max_context_results" => {
                self.max_context_results = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "max_search_results" => {
                self.max_search_results = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "dedupe_window_secs" => {
                self.dedupe_window_secs = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "vacuum_on_startup" => {
                self.vacuum_on_startup = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "checkpoint_interval_secs" => {
                self.checkpoint_interval_secs = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            _ => Err(ConfigError::InvalidKey(format!("storage.{}", key))),
        }
    }
}

impl SecurityConfig {
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "pqc_enabled" => Some(self.pqc_enabled.to_string()),
            "encryption_enabled" => Some(self.encryption_enabled.to_string()),
            "integrity_check_enabled" => Some(self.integrity_check_enabled.to_string()),
            "integrity_check_interval_secs" => Some(self.integrity_check_interval_secs.to_string()),
            "max_failed_auths" => Some(self.max_failed_auths.to_string()),
            "session_timeout_secs" => Some(self.session_timeout_secs.to_string()),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        match key {
            "pqc_enabled" => {
                self.pqc_enabled = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "encryption_enabled" => {
                self.encryption_enabled = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "integrity_check_enabled" => {
                self.integrity_check_enabled = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "integrity_check_interval_secs" => {
                self.integrity_check_interval_secs = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "max_failed_auths" => {
                self.max_failed_auths = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "session_timeout_secs" => {
                self.session_timeout_secs = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            _ => Err(ConfigError::InvalidKey(format!("security.{}", key))),
        }
    }
}

impl McpConfig {
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "tools" => Some(self.tools.join(", ")),
            "profile" => Some(self.profile.clone()),
            "auto_session" => Some(self.auto_session.to_string()),
            "auto_import" => Some(self.auto_import.to_string()),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        match key {
            "tools" => {
                self.tools = value.split(',').map(|s| s.trim().to_string()).collect();
                Ok(())
            }
            "profile" => {
                self.profile = value.to_string();
                Ok(())
            }
            "auto_session" => {
                self.auto_session = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "auto_import" => {
                self.auto_import = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            _ => Err(ConfigError::InvalidKey(format!("mcp.{}", key))),
        }
    }
}

impl SyncConfig {
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "enabled" => Some(self.enabled.to_string()),
            "remote_url" => self.remote_url.clone(),
            "sync_interval_secs" => Some(self.sync_interval_secs.to_string()),
            "conflict_resolution" => Some(format!("{:?}", self.conflict_resolution)),
            "enrolled_projects" => Some(self.enrolled_projects.join(", ")),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        match key {
            "enabled" => {
                self.enabled = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "remote_url" => {
                self.remote_url = Some(value.to_string());
                Ok(())
            }
            "sync_interval_secs" => {
                self.sync_interval_secs = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "conflict_resolution" => {
                self.conflict_resolution = match value {
                    "local_wins" => ConflictResolution::LocalWins,
                    "remote_wins" => ConflictResolution::RemoteWins,
                    "newest_wins" => ConflictResolution::NewestWins,
                    "manual" => ConflictResolution::Manual,
                    _ => return Err(ConfigError::InvalidValue(key.to_string())),
                };
                Ok(())
            }
            "enrolled_projects" => {
                self.enrolled_projects = value.split(',').map(|s| s.trim().to_string()).collect();
                Ok(())
            }
            _ => Err(ConfigError::InvalidKey(format!("sync.{}", key))),
        }
    }
}

impl LoggingConfig {
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "level" => Some(self.level.clone()),
            "format" => Some(self.format.clone()),
            "output" => Some(self.output.clone()),
            "max_size_mb" => Some(self.rotation.max_size_mb.to_string()),
            "max_files" => Some(self.rotation.max_files.to_string()),
            "compress" => Some(self.rotation.compress.to_string()),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        match key {
            "level" => {
                self.level = value.to_string();
                Ok(())
            }
            "format" => {
                self.format = value.to_string();
                Ok(())
            }
            "output" => {
                self.output = value.to_string();
                Ok(())
            }
            "max_size_mb" => {
                self.rotation.max_size_mb = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "max_files" => {
                self.rotation.max_files = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            "compress" => {
                self.rotation.compress = value
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue(key.to_string()))?;
                Ok(())
            }
            _ => Err(ConfigError::InvalidKey(format!("logging.{}", key))),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// FILE UTILITIES
// ═══════════════════════════════════════════════════════════════════════════

fn default_data_dir() -> String {
    if let Ok(home) = std::env::var("HOME") {
        format!("{}/.synapsis", home)
    } else {
        ".synapsis".to_string()
    }
}

fn file_exists(path: &str) -> bool {
    std::fs::metadata(path).is_ok()
}

fn read_file(path: &str) -> Result<String, ConfigError> {
    std::fs::read_to_string(path).map_err(|_| ConfigError::IoError(path.to_string()))
}

fn write_file(path: &str, content: &str) -> Result<(), ConfigError> {
    // Ensure directory exists
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).map_err(|_| ConfigError::IoError(path.to_string()))?;
    }

    std::fs::write(path, content).map_err(|_| ConfigError::IoError(path.to_string()))
}

// ═══════════════════════════════════════════════════════════════════════════
// ERRORS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum ConfigError {
    IoError(String),
    ParseError(String),
    InvalidKey(String),
    InvalidValue(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(path) => write!(f, "I/O error accessing: {}", path),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::InvalidKey(key) => write!(f, "Invalid configuration key: {}", key),
            Self::InvalidValue(key) => write!(f, "Invalid value for key: {}", key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.port, DEFAULT_PORT);
        assert_eq!(
            config.storage.max_observation_length,
            DEFAULT_MAX_OBS_LENGTH
        );
    }

    #[test]
    fn test_toml_roundtrip() {
        let config = Config::default();
        let toml = config.to_toml();
        let parsed = Config::parse(&toml).unwrap();
        assert_eq!(parsed.server.port, config.server.port);
    }

    #[test]
    fn test_get_set() {
        let mut config = Config::default();
        config.set("server.port", "9000").unwrap();
        assert_eq!(config.get("server.port"), Some("9000".to_string()));
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
[general]
data_dir = "/custom/path"

[server]
port = 8080
"#;
        let config = Config::parse(toml).unwrap();
        assert_eq!(config.general.data_dir, "/custom/path");
        assert_eq!(config.server.port, 8080);
    }
}
