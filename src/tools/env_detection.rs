//! Environment Detection - Auto-detect installed CLIs, TUIs, and IDEs
//! Supports MCP-compatible tools auto-configuration

use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

const KNOWN_CLIS: &[(&str, &str, Option<&str>)] = &[
    // AI Coding Assistants (MCP-compatible)
    ("qwen", "qwen-code", Some(".qwen")),
    ("qwen-code", "qwen-code", Some(".qwen")),
    ("opencode", "opencode", Some(".opencode")),
    ("claude", "claude", Some(".claude")),
    ("claude-code", "claude-code", Some(".claude")),
    ("gemini", "gemini-cli", Some(".gemini")),
    ("gemini-cli", "gemini-cli", Some(".gemini")),
    ("cline", "cline", Some(".config/rooveterinaryinc.roo-cline")),
    (
        "roo-cline",
        "roo-cline",
        Some(".config/rooveterinaryinc.roo-cline"),
    ),
    ("kilo", "kilo-code", Some(".config/kilocode.kilo-code")),
    ("kilo-code", "kilo-code", Some(".config/kilocode.kilo-code")),
    ("cursor", "cursor", Some(".config/Cursor")),
    ("windsurf", "windsurf", Some(".config/windsurf")),
    // AI CLI Tools
    ("ollama", "ollama", None),
    ("llama", "llama", None),
    ("lm-studio", "lm-studio", Some(".cache/lm-studio")),
    ("text-gen-webui", "text-gen-webui", None),
    ("koboldcpp", "koboldcpp", None),
    ("llm", "llm", None),
    // Dev Tools
    ("docker", "docker", None),
    ("kubectl", "kubectl", None),
    ("terraform", "terraform", None),
    ("ansible", "ansible", None),
    ("git", "git", None),
    ("npm", "npm", None),
    ("pnpm", "pnpm", None),
    ("yarn", "yarn", None),
    ("bun", "bun", None),
    ("deno", "deno", None),
    // Cloud CLIs
    ("aws", "aws", Some(".aws")),
    ("gcloud", "gcloud", Some(".config/gcloud")),
    ("az", "azure-cli", Some(".azure")),
    ("doctl", "doctl", None),
    ("linode", "linode-cli", None),
    // Security Tools
    ("nmap", "nmap", None),
    ("sqlmap", "sqlmap", None),
    ("hydra", "hydra", None),
    ("john", "john", None),
    ("hashcat", "hashcat", None),
    ("msfconsole", "msfconsole", None),
    ("nikto", "nikto", None),
    ("zap", "zaproxy", None),
];

const KNOWN_TUIS: &[(&str, &str)] = &[
    // File Managers
    ("yazi", "yazi"),
    ("ranger", "ranger"),
    ("nnn", "nnn"),
    ("fzf", "fzf"),
    // Terminal Multiplexers
    ("tmux", "tmux"),
    ("zellij", "zellij"),
    ("screen", "screen"),
    // System Monitors
    ("btop", "btop"),
    ("htop", "htop"),
    ("bottom", "bottom"),
    ("glances", "glances"),
    ("bashtop", "bashtop"),
    // Git TUIs
    ("lazygit", "lazygit"),
    ("tig", "tig"),
    ("gitui", "gitui"),
    // Network
    ("bandwhich", "bandwhich"),
    ("curlie", "curlie"),
    ("httpie", "httpie"),
    // Database TUIs
    ("pgcli", "pgcli"),
    ("mycli", "mycli"),
    ("usql", "usql"),
    ("litecli", "litecli"),
    // Other TUIs
    ("zoxide", "zoxide"),
    ("xplr", "xplr"),
    ("feh", "feh"),
    ("viu", "viu"),
];

const KNOWN_IDES: &[(&str, &str, Option<&str>)] = &[
    // VSCode-based
    ("vscode", "code", Some(".vscode")),
    ("vscode-oss", "code-oss", Some(".vscode-oss")),
    ("vscodium", "codium", Some(".config/VSCodium")),
    // JetBrains
    ("idea", "idea", Some(".config/JetBrains")),
    (
        "intellij",
        "intellij-idea-ultimate",
        Some(".config/JetBrains"),
    ),
    ("pycharm", "pycharm", Some(".config/JetBrains")),
    ("webstorm", "webstorm", Some(".config/JetBrains")),
    ("goland", "goland", Some(".config/JetBrains")),
    ("rustrover", "rust-rover", Some(".config/JetBrains")),
    ("datagrip", "datagrip", Some(".config/JetBrains")),
    ("clion", "clion", Some(".config/JetBrains")),
    ("phpstorm", "phpstorm", Some(".config/JetBrains")),
    ("rubymine", "rubymine", Some(".config/JetBrains")),
    ("appcode", "appcode", Some(".config/JetBrains")),
    ("macron", "macron", Some(".config/JetBrains")),
    ("rider", "rider", Some(".config/JetBrains")),
    (
        "gateway",
        "jetbrains-gateway",
        Some(".config/jetbrains-gateway"),
    ),
    // Modern Editors
    ("zed", "zed", Some(".config/zed")),
    ("helix", "hx", Some(".config/helix")),
    ("neovim", "nvim", Some(".config/nvim")),
    ("vim", "vim", None),
    ("emacs", "emacs", Some(".emacs.d")),
    // Cloud IDEs
    ("gitpod", "gitpod", Some(".gitpod")),
    ("coder", "coder", None),
    ("code-server", "code-server", None),
    // Specialized
    ("rstudio", "rstudio", Some(".config/RStudio")),
    ("julia", "julia", None),
    ("matlab", "matlab", None),
    // Language Servers
    ("rust-analyzer", "rust-analyzer", None),
    ("pylsp", "pylsp", None),
    ("clangd", "clangd", None),
    ("gopls", "gopls", None),
    ("tsserver", "typescript-language-server", None),
    ("svelte", "svelte-language-server", None),
];

fn check_command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn check_path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

fn get_home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home/methodwhite"))
}

fn detect_cli(name: &str, binary: &str) -> Option<HashMap<String, serde_json::Value>> {
    if check_command_exists(binary) {
        let mut info = HashMap::new();
        info.insert("name".to_string(), json!(name));
        info.insert("binary".to_string(), json!(binary));
        info.insert("type".to_string(), json!("cli"));

        if let Ok(output) = Command::new(binary).arg("--version").output() {
            if output.status.success() {
                let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                info.insert("version".to_string(), json!(version_str));
            }
        }

        return Some(info);
    }
    None
}

fn detect_tui(name: &str, binary: &str) -> Option<HashMap<String, serde_json::Value>> {
    if check_command_exists(binary) {
        let mut info = HashMap::new();
        info.insert("name".to_string(), json!(name));
        info.insert("binary".to_string(), json!(binary));
        info.insert("type".to_string(), json!("tui"));

        if let Ok(output) = Command::new(binary).arg("--version").output() {
            if output.status.success() {
                let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                info.insert("version".to_string(), json!(version_str));
            }
        }

        return Some(info);
    }
    None
}

fn detect_ide(name: &str, binary: &str) -> Option<HashMap<String, serde_json::Value>> {
    if check_command_exists(binary) {
        let mut info = HashMap::new();
        info.insert("name".to_string(), json!(name));
        info.insert("binary".to_string(), json!(binary));
        info.insert("type".to_string(), json!("ide"));

        // Try to get version
        if let Ok(output) = Command::new(binary).arg("--version").output() {
            if output.status.success() {
                let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                info.insert("version".to_string(), json!(version_str));
            }
        }

        return Some(info);
    }

    // Check common IDE paths
    let home = get_home_dir();
    let ide_paths = vec![
        home.join(format!(".local/share/{}", binary)),
        home.join(format!(".vscode-oss/{}", binary)),
        home.join(format!(".config/{}", binary)),
    ];

    for path in ide_paths {
        if path.exists() {
            let mut info = HashMap::new();
            info.insert("name".to_string(), json!(name));
            info.insert("type".to_string(), json!("ide"));
            info.insert(
                "path".to_string(),
                json!(path.to_string_lossy().to_string()),
            );
            return Some(info);
        }
    }

    None
}

pub fn detect_environment() -> Result<serde_json::Value> {
    let mut clis = Vec::new();
    let mut tais = Vec::new();
    let mut ides = Vec::new();

    // Detect CLIs
    for (name, binary, _) in KNOWN_CLIS {
        if let Some(info) = detect_cli(name, binary) {
            clis.push(info);
        }
    }

    // Detect TUIs
    for (name, binary) in KNOWN_TUIS {
        if let Some(info) = detect_tui(name, binary) {
            tais.push(info);
        }
    }

    // Detect IDEs
    for (name, binary, _) in KNOWN_IDES {
        if let Some(info) = detect_ide(name, binary) {
            ides.push(info);
        }
    }

    Ok(json!({
        "status": "success",
        "clis": clis,
        "tuis": tais,
        "ides": ides,
        "total": clis.len() + tais.len() + ides.len()
    }))
}

pub fn detect_mcp_compatible() -> Result<serde_json::Value> {
    let mut mcp_compatible = Vec::new();

    // Check which detected CLIs support MCP
    for (name, binary, config_dir) in KNOWN_CLIS {
        if check_command_exists(binary) {
            // Check if CLI has MCP support or config
            let home = get_home_dir();

            // Build config paths based on known config directories
            let mut mcp_configs: Vec<PathBuf> = vec![
                home.join(format!(".config/{}/mcp.json", name)),
                home.join(format!(".{}/mcp.json", name)),
                home.join(format!(".config/{}/mcp-settings.json", name)),
            ];

            // Add known config directories from the tuple
            if let Some(dir) = config_dir {
                mcp_configs.push(home.join(format!("{}/mcp.json", dir)));
                mcp_configs.push(home.join(format!("{}/mcp_settings.json", dir)));
            }

            for config_path in mcp_configs {
                if config_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&config_path) {
                        if content.contains("synapsis") || content.contains("mcpServers") {
                            let mut info = HashMap::new();
                            info.insert("name".to_string(), json!(name));
                            info.insert("binary".to_string(), json!(binary));
                            info.insert(
                                "config_path".to_string(),
                                json!(config_path.to_string_lossy().to_string()),
                            );
                            info.insert("mcp_configured".to_string(), json!(true));
                            mcp_compatible.push(info);
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(json!({
        "status": "success",
        "mcp_compatible": mcp_compatible,
        "count": mcp_compatible.len()
    }))
}

pub fn get_auto_connect_config() -> Result<serde_json::Value> {
    let mut configs = Vec::new();
    let home = get_home_dir();

    // Generate MCP config for each detected CLI that supports MCP
    for (name, binary, config_dir) in KNOWN_CLIS {
        if check_command_exists(binary) {
            let config = json!({
                "name": name,
                "binary": binary,
                "config_dir": config_dir.unwrap_or(""),
                "mcpServers": {
                    "synapsis": {
                        "type": "stdio",
                        "command": "synapsis",
                        "args": ["mcp"],
                        "enabled": true,
                        "autoStart": true,
                        "config": {
                            "project_key": name,
                            "heartbeat_interval": 30,
                            "auto_save_context": true,
                            "tools": [
                                "memory_add",
                                "memory_search",
                                "memory_timeline",
                                "agent_session_register",
                                "agent_heartbeat",
                                "agents_active",
                                "task_queue_create",
                                "task_claim",
                                "env_detection"
                            ]
                        }
                    }
                }
            });
            configs.push(config);
        }
    }

    Ok(json!({
        "status": "success",
        "configs": configs,
        "count": configs.len()
    }))
}

pub fn mcp_tools() -> &'static str {
    r#"
    {
        "name": "env_detection",
        "description": "Detect installed CLIs, TUIs, and IDEs that can connect to Synapsis MCP",
        "inputSchema": {
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["all", "mcp_compatible", "auto_config"],
                    "default": "all"
                }
            }
        }
    }
    "#
}

pub fn handle_env_detection(mode: Option<&str>) -> Result<serde_json::Value> {
    match mode {
        Some("mcp_compatible") => detect_mcp_compatible(),
        Some("auto_config") => get_auto_connect_config(),
        _ => detect_environment(),
    }
}
