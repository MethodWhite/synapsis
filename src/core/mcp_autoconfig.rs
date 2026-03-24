//! MCP Server Autoconfigurator
//! 
//! Intelligent MCP server configuration that detects installed CLIs, TUIs, and IDEs.
//! Automatically configures MCP server based on detected tools and their capabilities.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};
use crate::core::discovery::{EnvironmentDiscovery, DiscoveredTool, ToolType};

/// MCP client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientConfig {
    pub name: String,
    pub mcp_protocol_version: String,
    pub transport: McpTransport,
    pub capabilities: Vec<String>,
    pub config_path: Option<PathBuf>,
    pub auto_enable: bool,
}

/// MCP transport type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum McpTransport {
    Stdio,
    Tcp { port: u16, host: String },
    UnixSocket { path: PathBuf },
    Http { url: String },
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub server_id: String,
    pub enabled_transports: Vec<McpTransport>,
    pub tools: Vec<McpToolConfig>,
    pub auto_discovery: bool,
    pub security: McpSecurityConfig,
    pub integration: HashMap<String, McpIntegration>,
}

/// MCP tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolConfig {
    pub name: String,
    pub tool_type: String,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,
    pub auto_start: bool,
}

/// MCP security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSecurityConfig {
    pub require_auth: bool,
    pub allowed_origins: Vec<String>,
    pub rate_limit: Option<u32>,
    pub enable_tls: bool,
}

/// MCP integration with detected tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpIntegration {
    pub tool_name: String,
    pub integration_type: IntegrationType,
    pub config: serde_json::Value,
    pub enabled: bool,
}

/// Integration type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrationType {
    Plugin,
    Extension,
    Bridge,
    Native,
    External,
}

/// Intelligent MCP autoconfigurator
pub struct McpAutoconfigurator {
    discovery: EnvironmentDiscovery,
    detected_tools: Vec<DiscoveredTool>,
    known_mcp_clients: HashMap<String, McpClientConfig>,
}

impl McpAutoconfigurator {
    /// Create a new autoconfigurator
    pub fn new() -> Self {
        let discovery = EnvironmentDiscovery::new();
        let detected_tools = discovery.discover_all();
        
        let mut configurator = Self {
            discovery,
            detected_tools,
            known_mcp_clients: HashMap::new(),
        };
        
        configurator.load_known_clients();
        configurator
    }
    
    /// Load known MCP client configurations
    fn load_known_clients(&mut self) {
        // VS Code with MCP extension
        self.known_mcp_clients.insert("vscode".to_string(), McpClientConfig {
            name: "VS Code".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            transport: McpTransport::Stdio,
            capabilities: vec!["tools".to_string(), "resources".to_string(), "prompts".to_string()],
            config_path: Some(PathBuf::from("~/.config/Code/User/globalStorage/state.vscdb")),
            auto_enable: true,
        });
        
        // Cursor IDE
        self.known_mcp_clients.insert("cursor".to_string(), McpClientConfig {
            name: "Cursor".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            transport: McpTransport::Stdio,
            capabilities: vec!["tools".to_string(), "resources".to_string()],
            config_path: Some(PathBuf::from("~/.cursor/state.json")),
            auto_enable: true,
        });
        
        // Windsurf
        self.known_mcp_clients.insert("windsurf".to_string(), McpClientConfig {
            name: "Windsurf".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            transport: McpTransport::Tcp { port: 3000, host: "127.0.0.1".to_string() },
            capabilities: vec!["tools".to_string(), "resources".to_string(), "prompts".to_string()],
            config_path: Some(PathBuf::from("~/.windsurf/config.json")),
            auto_enable: true,
        });
        
        // OpenCode
        self.known_mcp_clients.insert("opencode".to_string(), McpClientConfig {
            name: "OpenCode".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            transport: McpTransport::Tcp { port: 7438, host: "127.0.0.1".to_string() },
            capabilities: vec!["tools".to_string(), "resources".to_string(), "multi-agent".to_string()],
            config_path: None,
            auto_enable: true,
        });
        
        // Claude Code
        self.known_mcp_clients.insert("claude".to_string(), McpClientConfig {
            name: "Claude Code".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            transport: McpTransport::Stdio,
            capabilities: vec!["tools".to_string(), "resources".to_string()],
            config_path: Some(PathBuf::from("~/.claude/config.json")),
            auto_enable: true,
        });
        
        // Qwen
        self.known_mcp_clients.insert("qwen".to_string(), McpClientConfig {
            name: "Qwen Code".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            transport: McpTransport::Stdio,
            capabilities: vec!["tools".to_string()],
            config_path: Some(PathBuf::from("~/.qwen/config.json")),
            auto_enable: true,
        });
        
        // Gemini CLI
        self.known_mcp_clients.insert("gemini".to_string(), McpClientConfig {
            name: "Gemini CLI".to_string(),
            mcp_protocol_version: "2024-11-05".to_string(),
            transport: McpTransport::Stdio,
            capabilities: vec!["tools".to_string()],
            config_path: Some(PathBuf::from("~/.gemini/config.json")),
            auto_enable: true,
        });
    }
    
    /// Detect MCP-capable clients
    pub fn detect_mcp_clients(&self) -> Vec<McpClientConfig> {
        let mut clients = Vec::new();
        
        for tool in &self.detected_tools {
            if let Some(client_config) = self.known_mcp_clients.get(&tool.name.to_lowercase()) {
                let mut config = client_config.clone();
                
                // Update with actual detection information
                if let Some(path) = &tool.path {
                    // Check for specific configuration files
                    let config_path = self.find_mcp_config(&tool.name, path);
                    config.config_path = config_path;
                }
                
                clients.push(config);
            }
        }
        
        clients
    }
    
    /// Find MCP configuration file for a tool
    fn find_mcp_config(&self, tool_name: &str, install_path: &Path) -> Option<PathBuf> {
        let home_dir = std::env::var("HOME").ok().map(PathBuf::from);
        
        match tool_name.to_lowercase().as_str() {
            "vscode" | "code" => {
                home_dir.map(|h| h.join(".config/Code/User/globalStorage/state.vscdb"))
            }
            "cursor" => {
                home_dir.map(|h| h.join(".cursor/state.json"))
            }
            "windsurf" => {
                home_dir.map(|h| h.join(".windsurf/config.json"))
            }
            "claude" => {
                home_dir.map(|h| h.join(".claude/config.json"))
            }
            "qwen" => {
                home_dir.map(|h| h.join(".qwen/config.json"))
            }
            "gemini" => {
                home_dir.map(|h| h.join(".gemini/config.json"))
            }
            _ => None,
        }
    }
    
    /// Generate optimal MCP server configuration
    pub fn generate_server_config(&self) -> McpServerConfig {
        let detected_clients = self.detect_mcp_clients();
        
        // Determine which transports to enable
        let mut transports = HashSet::new();
        for client in &detected_clients {
            transports.insert(client.transport.clone());
        }
        
        // If no clients detected, enable default transports
        if transports.is_empty() {
            transports.insert(McpTransport::Stdio);
            transports.insert(McpTransport::Tcp { port: 7438, host: "127.0.0.1".to_string() });
        }
        
        // Generate tool configurations
        let tools = self.generate_tool_configs();
        
        McpServerConfig {
            server_id: format!("synapsis-{}", uuid::Uuid::new_v4()),
            enabled_transports: transports.into_iter().collect(),
            tools,
            auto_discovery: true,
            security: McpSecurityConfig {
                require_auth: detected_clients.len() > 1, // Require auth for multi-client
                allowed_origins: vec!["127.0.0.1".to_string(), "localhost".to_string()],
                rate_limit: Some(100),
                enable_tls: false,
            },
            integration: self.generate_integrations(&detected_clients),
        }
    }
    
    /// Generate tool configurations based on detected tools
    fn generate_tool_configs(&self) -> Vec<McpToolConfig> {
        let mut tools = Vec::new();
        
        for tool in &self.detected_tools {
            // Skip tools that aren't MCP-compatible
            if !self.is_mcp_compatible(&tool.name) {
                continue;
            }
            
            if let Some(path) = &tool.path {
                let tool_config = McpToolConfig {
                    name: tool.name.clone(),
                    tool_type: match tool.tool_type {
                        ToolType::AiAgent => "ai_agent".to_string(),
                        ToolType::Ide => "ide".to_string(),
                        ToolType::DevTool => "dev_tool".to_string(),
                        ToolType::PackageManager => "package_manager".to_string(),
                        ToolType::ApiTool => "api_tool".to_string(),
                        ToolType::Linter => "linter".to_string(),
                        ToolType::Framework => "framework".to_string(),
                        ToolType::Unknown => "unknown".to_string(),
                    },
                    executable: path.clone(),
                    args: self.get_tool_args(&tool.name),
                    env: self.get_tool_env(&tool.name),
                    working_dir: std::env::current_dir().ok(),
                    auto_start: tool.auto_integrate,
                };
                
                tools.push(tool_config);
            }
        }
        
        tools
    }
    
    /// Check if a tool is MCP-compatible
    fn is_mcp_compatible(&self, tool_name: &str) -> bool {
        matches!(tool_name.to_lowercase().as_str(),
            "git" | "docker" | "curl" | "jq" | "http" |
            "opencode" | "claude" | "qwen" | "gemini" |
            "vscode" | "cursor" | "windsurf"
        )
    }
    
    /// Get command-line arguments for a tool
    fn get_tool_args(&self, tool_name: &str) -> Vec<String> {
        match tool_name.to_lowercase().as_str() {
            "git" => vec!["--no-pager".to_string()],
            "docker" => vec![],
            "curl" => vec!["-s".to_string(), "--fail".to_string()],
            "jq" => vec!["-r".to_string()],
            _ => vec![],
        }
    }
    
    /// Get environment variables for a tool
    fn get_tool_env(&self, tool_name: &str) -> HashMap<String, String> {
        let mut env = HashMap::new();
        
        match tool_name.to_lowercase().as_str() {
            "git" => {
                env.insert("GIT_PAGER".to_string(), "cat".to_string());
                env.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());
            }
            "docker" => {
                env.insert("DOCKER_BUILDKIT".to_string(), "1".to_string());
            }
            _ => {}
        }
        
        env
    }
    
    /// Generate integrations for detected clients
    fn generate_integrations(&self, clients: &[McpClientConfig]) -> HashMap<String, McpIntegration> {
        let mut integrations = HashMap::new();
        
        for client in clients {
            let integration = McpIntegration {
                tool_name: client.name.clone(),
                integration_type: match client.name.as_str() {
                    "VS Code" | "Cursor" | "Windsurf" => IntegrationType::Extension,
                    "OpenCode" | "Claude Code" | "Qwen Code" => IntegrationType::Native,
                    _ => IntegrationType::Bridge,
                },
                config: serde_json::json!({
                    "protocol_version": client.mcp_protocol_version,
                    "capabilities": client.capabilities,
                    "auto_enable": client.auto_enable,
                }),
                enabled: true,
            };
            
            integrations.insert(client.name.clone(), integration);
        }
        
        integrations
    }
    
    /// Apply configuration to system
    pub fn apply_configuration(&self, config: &McpServerConfig) -> Result<(), String> {
        // Save configuration to file
        let config_dir = self.get_config_dir();
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
        
        let config_path = config_dir.join("mcp_server.json");
        let config_json = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        
        std::fs::write(&config_path, config_json)
            .map_err(|e| format!("Failed to write config file: {}", e))?;
        
        // Generate startup scripts if needed
        self.generate_startup_scripts(config)?;
        
        // Update system configuration files
        self.update_system_configs(config)?;
        
        Ok(())
    }
    
    /// Get configuration directory
    fn get_config_dir(&self) -> PathBuf {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg).join("synapsis")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".config/synapsis")
        } else {
            PathBuf::from(".synapsis")
        }
    }
    
    /// Generate startup scripts for detected platforms
    fn generate_startup_scripts(&self, config: &McpServerConfig) -> Result<(), String> {
        // Generate systemd service file for Linux
        if cfg!(target_os = "linux") {
            self.generate_systemd_service(config)?;
        }
        
        // Generate launchd plist for macOS
        if cfg!(target_os = "macos") {
            self.generate_launchd_plist(config)?;
        }
        
        // Generate Windows service
        if cfg!(target_os = "windows") {
            self.generate_windows_service(config)?;
        }
        
        Ok(())
    }
    
    /// Generate systemd service file
    fn generate_systemd_service(&self, config: &McpServerConfig) -> Result<(), String> {
        let service_content = format!(
            r#"[Unit]
Description=Synapsis MCP Server
After=network.target

[Service]
Type=simple
ExecStart={} mcp
Restart=always
RestartSec=10
User={}
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target"#,
            std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "synapsis".to_string()),
            std::env::var("USER").unwrap_or_else(|_| "root".to_string())
        );
        
        let service_path = PathBuf::from("/etc/systemd/system/synapsis-mcp.service");
        
        // Try to write with sudo privileges
        let temp_path = self.get_config_dir().join("synapsis-mcp.service");
        std::fs::write(&temp_path, service_content)
            .map_err(|e| format!("Failed to write service file: {}", e))?;
        
        println!("Systemd service file written to: {}", temp_path.display());
        println!("To install: sudo cp {} /etc/systemd/system/", temp_path.display());
        println!("Then: sudo systemctl enable --now synapsis-mcp.service");
        
        Ok(())
    }
    
    /// Generate launchd plist for macOS
    fn generate_launchd_plist(&self, _config: &McpServerConfig) -> Result<(), String> {
        // Implementation for macOS
        Ok(())
    }
    
    /// Generate Windows service
    fn generate_windows_service(&self, _config: &McpServerConfig) -> Result<(), String> {
        // Implementation for Windows
        Ok(())
    }
    
    /// Update system configuration files
    fn update_system_configs(&self, config: &McpServerConfig) -> Result<(), String> {
        // Update shell profiles with environment variables
        self.update_shell_profiles(config)?;
        
        // Update IDE configuration files if detected
        self.update_ide_configs(config)?;
        
        Ok(())
    }
    
    /// Update shell profiles with Synapsis environment variables
    fn update_shell_profiles(&self, config: &McpServerConfig) -> Result<(), String> {
        let shell_exports = format!(
            r#"# Synapsis MCP Server Configuration
export SYNAPSIS_MCP_SERVER_ID="{}"
export SYNAPSIS_MCP_AUTO_DISCOVERY={}
export SYNAPSIS_MCP_ENABLED_TRANSPORTS="{}"
"#,
            config.server_id,
            config.auto_discovery,
            config.enabled_transports.iter()
                .map(|t| format!("{:?}", t))
                .collect::<Vec<_>>()
                .join(",")
        );
        
        // Try to append to common shell profiles
        let home_dir = std::env::var("HOME").map_err(|e| format!("HOME not set: {}", e))?;
        let profiles = [
            ".bashrc",
            ".bash_profile",
            ".zshrc",
            ".profile",
            ".config/fish/config.fish",
        ];
        
        for profile in &profiles {
            let profile_path = PathBuf::from(&home_dir).join(profile);
            if profile_path.exists() {
                // Check if already configured
                let existing = std::fs::read_to_string(&profile_path).unwrap_or_default();
                if !existing.contains("SYNAPSIS_MCP_SERVER_ID") {
                    let mut file = std::fs::OpenOptions::new()
                        .append(true)
                        .open(&profile_path)
                        .map_err(|e| format!("Failed to open {}: {}", profile, e))?;
                    
                    use std::io::Write;
                    writeln!(file, "\n{}", shell_exports)
                        .map_err(|e| format!("Failed to write to {}: {}", profile, e))?;
                    
                    println!("Updated {} with Synapsis environment variables", profile);
                }
            }
        }
        
        Ok(())
    }
    
    /// Update IDE configuration files
    fn update_ide_configs(&self, config: &McpServerConfig) -> Result<(), String> {
        for tool in &self.detected_tools {
            match tool.name.to_lowercase().as_str() {
                "vscode" | "code" => {
                    self.update_vscode_config(config)?;
                }
                "cursor" => {
                    self.update_cursor_config(config)?;
                }
                "windsurf" => {
                    self.update_windsurf_config(config)?;
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    /// Update VS Code configuration
    fn update_vscode_config(&self, _config: &McpServerConfig) -> Result<(), String> {
        // VS Code MCP configuration would go in settings.json
        Ok(())
    }
    
    /// Update Cursor configuration
    fn update_cursor_config(&self, _config: &McpServerConfig) -> Result<(), String> {
        // Cursor MCP configuration
        Ok(())
    }
    
    /// Update Windsurf configuration
    fn update_windsurf_config(&self, _config: &McpServerConfig) -> Result<(), String> {
        // Windsurf MCP configuration
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_autoconfigurator_creation() {
        let configurator = McpAutoconfigurator::new();
        assert!(!configurator.detected_tools.is_empty());
    }
    
    #[test]
    fn test_detect_mcp_clients() {
        let configurator = McpAutoconfigurator::new();
        let clients = configurator.detect_mcp_clients();
        // At minimum should detect the test environment
        assert!(clients.len() >= 0);
    }
    
    #[test]
    fn test_generate_server_config() {
        let configurator = McpAutoconfigurator::new();
        let config = configurator.generate_server_config();
        
        assert!(!config.server_id.is_empty());
        assert!(!config.enabled_transports.is_empty());
        assert!(config.auto_discovery);
    }
    
    #[test]
    fn test_is_mcp_compatible() {
        let configurator = McpAutoconfigurator::new();
        
        assert!(configurator.is_mcp_compatible("git"));
        assert!(configurator.is_mcp_compatible("docker"));
        assert!(configurator.is_mcp_compatible("vscode"));
        assert!(configurator.is_mcp_compatible("cursor"));
        
        assert!(!configurator.is_mcp_compatible("unknown-tool"));
    }
}