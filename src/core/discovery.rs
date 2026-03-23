//! Synapsis Environment Discovery - Auto-detect available tools/agents/IDEs

use crate::domain::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredTool {
    pub name: String,
    pub tool_type: ToolType,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub capabilities: Vec<String>,
    pub is_available: bool,
    pub auto_integrate: bool,
    pub discovered_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolType {
    AiAgent,
    Ide,
    DevTool,
    PackageManager,
    ApiTool,
    Linter,
    Framework,
    Unknown,
}

impl ToolType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolType::AiAgent => "ai_agent",
            ToolType::Ide => "ide",
            ToolType::DevTool => "dev_tool",
            ToolType::PackageManager => "package_manager",
            ToolType::ApiTool => "api_tool",
            ToolType::Linter => "linter",
            ToolType::Framework => "framework",
            ToolType::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationResult {
    pub tool_name: String,
    pub connected: bool,
    pub worker_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryScan {
    pub total_found: usize,
    pub by_type: HashMap<String, usize>,
    pub new_tools: Vec<DiscoveredTool>,
    pub scan_time_ms: u64,
}

pub struct EnvironmentDiscovery {
    scan_cache: HashMap<String, DiscoveredTool>,
    _last_scan: Option<i64>,
}

impl EnvironmentDiscovery {
    pub fn new() -> Self {
        Self {
            scan_cache: HashMap::new(),
            _last_scan: None,
        }
    }

    pub fn discover_all(&self) -> Vec<DiscoveredTool> {
        let mut tools = Vec::new();
        tools.extend(self.check_ide());
        tools.extend(self.check_cli_agents());
        tools.extend(self.check_shell_tools());
        tools.extend(self.check_apis());
        tools.extend(self.check_frameworks());
        tools
    }

    pub fn check_ide(&self) -> Vec<DiscoveredTool> {
        let mut tools = Vec::new();
        let now = Timestamp::now().0;

        let ide_tools = [
            ("cursor", vec!["code completion", "chat", "terminal"]),
            ("windsurf", vec!["code completion", "chat", "terminal"]),
            ("code", vec!["code completion", "debugging", "extensions"]),
            ("subl", vec!["code completion", "plugins"]),
            ("vim", vec!["text editing", "terminal"]),
            ("nvim", vec!["text editing", "terminal", "plugins"]),
            ("emacs", vec!["text editing", "org-mode"]),
            ("atom", vec!["code completion", "plugins"]),
        ];

        for (name, caps) in ide_tools.iter() {
            if let Some(tool) = self.check_tool(name) {
                tools.push(DiscoveredTool {
                    name: tool.0,
                    tool_type: ToolType::Ide,
                    path: tool.1,
                    version: tool.2,
                    capabilities: caps.iter().map(|s| s.to_string()).collect(),
                    is_available: true,
                    auto_integrate: true,
                    discovered_at: now,
                });
            }
        }

        tools
    }

    pub fn check_cli_agents(&self) -> Vec<DiscoveredTool> {
        let mut tools = Vec::new();
        let now = Timestamp::now().0;

        let agent_tools = [
            (
                "opencode",
                vec!["code completion", "chat", "tool-use", "multi-file"],
            ),
            ("claude", vec!["code completion", "chat", "tool-use"]),
            ("qwen", vec!["code completion", "chat"]),
            ("gemini", vec!["code completion", "chat"]),
            ("mistral", vec!["code completion", "chat"]),
            ("ollama", vec!["llm-local", "code completion"]),
            ("llama", vec!["llm-local", "code completion"]),
            ("copilot", vec!["code completion", "chat"]),
        ];

        for (name, caps) in agent_tools.iter() {
            if let Some(tool) = self.check_tool(name) {
                tools.push(DiscoveredTool {
                    name: tool.0,
                    tool_type: ToolType::AiAgent,
                    path: tool.1,
                    version: tool.2,
                    capabilities: caps.iter().map(|s| s.to_string()).collect(),
                    is_available: true,
                    auto_integrate: true,
                    discovered_at: now,
                });
            }
        }

        tools
    }

    pub fn check_shell_tools(&self) -> Vec<DiscoveredTool> {
        let mut tools = Vec::new();
        let now = Timestamp::now().0;

        let dev_tools = [
            ("git", vec!["version-control", "branching", "merge"]),
            ("docker", vec!["containers", "images", "orchestration"]),
            ("kubectl", vec!["kubernetes", "pods", "deployments"]),
            ("terraform", vec!["infrastructure", "IaC"]),
            ("ansible", vec!["automation", "configuration"]),
            ("make", vec!["build", "automation"]),
            ("cmake", vec!["build", "compilation"]),
            ("gcc", vec!["compilation", "build"]),
            ("rustc", vec!["compilation", "build"]),
            ("go", vec!["compilation", "build", "testing"]),
        ];

        for (name, caps) in dev_tools.iter() {
            if let Some(tool) = self.check_tool(name) {
                tools.push(DiscoveredTool {
                    name: tool.0,
                    tool_type: ToolType::DevTool,
                    path: tool.1,
                    version: tool.2,
                    capabilities: caps.iter().map(|s| s.to_string()).collect(),
                    is_available: true,
                    auto_integrate: false,
                    discovered_at: now,
                });
            }
        }

        tools
    }

    pub fn check_apis(&self) -> Vec<DiscoveredTool> {
        let mut tools = Vec::new();
        let now = Timestamp::now().0;

        let api_tools = [
            ("curl", vec!["http-requests", "rest", "api"]),
            ("wget", vec!["http-downloads", "rest"]),
            ("jq", vec!["json-processing", "parsing"]),
            ("http", vec!["http-requests", "rest"]),
            ("grpcurl", vec!["grpc", "api"]),
        ];

        for (name, caps) in api_tools.iter() {
            if let Some(tool) = self.check_tool(name) {
                tools.push(DiscoveredTool {
                    name: tool.0,
                    tool_type: ToolType::ApiTool,
                    path: tool.1,
                    version: tool.2,
                    capabilities: caps.iter().map(|s| s.to_string()).collect(),
                    is_available: true,
                    auto_integrate: false,
                    discovered_at: now,
                });
            }
        }

        tools
    }

    pub fn check_frameworks(&self) -> Vec<DiscoveredTool> {
        let mut tools = Vec::new();
        let now = Timestamp::now().0;
        let frameworks = [
            ("node", vec!["javascript", "runtime", "npm"]),
            ("npm", vec!["package-manager", "node"]),
            ("yarn", vec!["package-manager", "node"]),
            ("pnpm", vec!["package-manager", "node"]),
            ("cargo", vec!["package-manager", "rust"]),
            ("pip", vec!["package-manager", "python"]),
            ("poetry", vec!["package-manager", "python"]),
            ("composer", vec!["package-manager", "php"]),
            ("bundle", vec!["package-manager", "ruby"]),
        ];

        for (name, caps) in frameworks.iter() {
            if let Some(tool) = self.check_tool(name) {
                tools.push(DiscoveredTool {
                    name: tool.0,
                    tool_type: ToolType::PackageManager,
                    path: tool.1,
                    version: tool.2,
                    capabilities: caps.iter().map(|s| s.to_string()).collect(),
                    is_available: true,
                    auto_integrate: true,
                    discovered_at: now,
                });
            }
        }

        let linters = [
            ("ruff", vec!["linting", "python"]),
            ("eslint", vec!["linting", "javascript"]),
            ("prettier", vec!["formatting", "code"]),
            ("golangci-lint", vec!["linting", "go"]),
            ("rustfmt", vec!["formatting", "rust"]),
            ("black", vec!["formatting", "python"]),
            ("mypy", vec!["type-checking", "python"]),
        ];

        for (name, caps) in linters.iter() {
            if let Some(tool) = self.check_tool(name) {
                tools.push(DiscoveredTool {
                    name: tool.0,
                    tool_type: ToolType::Linter,
                    path: tool.1,
                    version: tool.2,
                    capabilities: caps.iter().map(|s| s.to_string()).collect(),
                    is_available: true,
                    auto_integrate: false,
                    discovered_at: now,
                });
            }
        }

        tools
    }

    fn check_tool(&self, name: &str) -> Option<(String, Option<PathBuf>, Option<String>)> {
        let output = Command::new("which").arg(name).output().ok()?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let version = self.get_version(name);
            return Some((name.to_string(), Some(PathBuf::from(path)), version));
        }

        None
    }

    fn get_version(&self, name: &str) -> Option<String> {
        let version_flags = ["--version", "-v", "version", "-V"];

        for flag in version_flags.iter() {
            let output = Command::new(name).arg(flag).output().ok()?;
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("unknown")
                    .trim()
                    .to_string();
                if !version.is_empty() && !version.contains("not found") {
                    return Some(version);
                }
            }
        }

        None
    }

    pub fn auto_integrate(&self, tool: &DiscoveredTool) -> Result<IntegrationResult> {
        if !tool.is_available {
            return Ok(IntegrationResult {
                tool_name: tool.name.clone(),
                connected: false,
                worker_id: None,
                error: Some("Tool not available".to_string()),
            });
        }

        match tool.tool_type {
            ToolType::AiAgent => Ok(IntegrationResult {
                tool_name: tool.name.clone(),
                connected: true,
                worker_id: Some(format!("{}-auto-{}", tool.name, Timestamp::now().0)),
                error: None,
            }),
            ToolType::Ide => Ok(IntegrationResult {
                tool_name: tool.name.clone(),
                connected: true,
                worker_id: None,
                error: None,
            }),
            _ => Ok(IntegrationResult {
                tool_name: tool.name.clone(),
                connected: true,
                worker_id: None,
                error: None,
            }),
        }
    }

    pub fn scan(&self) -> DiscoveryScan {
        let start = std::time::Instant::now();
        let tools = self.discover_all();

        let mut by_type: HashMap<String, usize> = HashMap::new();
        let mut new_tools = Vec::new();

        for tool in &tools {
            *by_type
                .entry(tool.tool_type.as_str().to_string())
                .or_insert(0) += 1;

            if !self.scan_cache.contains_key(&tool.name) {
                new_tools.push(tool.clone());
            }
        }

        DiscoveryScan {
            total_found: tools.len(),
            by_type,
            new_tools,
            scan_time_ms: start.elapsed().as_millis() as u64,
        }
    }
}

impl Default for EnvironmentDiscovery {
    fn default() -> Self {
        Self::new()
    }
}
