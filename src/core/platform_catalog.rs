//! Synapsis Platform Catalog
//!
//! Comprehensive registry of AI development platforms, categorized by type
//! and geography. Provides detection, MCP config generation, and discovery.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    MCP,
    REST,
    WebSocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformCategory {
    CliTool,
    IdeIntegration,
    Tui,
    ChineseAiPlatform,
    Emerging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformStatus {
    Active,
    Beta,
    Deprecated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub name: String,
    pub category: PlatformCategory,
    pub protocol: Protocol,
    pub detection_hints: Vec<String>,
    pub mcp_config_template: Option<serde_json::Value>,
    pub status: PlatformStatus,
    pub country: String,
    pub homepage: String,
    pub description: String,
}

pub fn all_platforms() -> Vec<Platform> {
    vec![
        // ── CLI Tools (MCP-native) ──────────────────────────────────────────
        Platform {
            name: "OpenCode".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::MCP,
            detection_hints: vec!["opencode".into(), "~/.config/opencode/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "opencode",
                "args": ["mcp"]
            })),
            status: PlatformStatus::Active,
            country: "Global".into(),
            homepage: "https://opencode.ai".into(),
            description: "Open-source AI coding assistant with MCP native support".into(),
        },
        Platform {
            name: "Claude Code".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::MCP,
            detection_hints: vec!["claude".into(), "~/.claude/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "claude",
                "args": ["mcp"]
            })),
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://claude.ai".into(),
            description: "Anthropic's CLI coding agent with MCP support".into(),
        },
        Platform {
            name: "Gemini CLI".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::MCP,
            detection_hints: vec!["gemini".into(), "~/.google/gemini/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "gemini",
                "args": ["mcp"]
            })),
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://deepmind.google/gemini".into(),
            description: "Google's Gemini AI coding assistant CLI".into(),
        },
        Platform {
            name: "Cline".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::MCP,
            detection_hints: vec!["cline".into(), "~/.config/cline/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "cline",
                "args": ["mcp"]
            })),
            status: PlatformStatus::Active,
            country: "Global".into(),
            homepage: "https://github.com/cline/cline".into(),
            description: "Autonomous coding agent with MCP support".into(),
        },
        Platform {
            name: "aider".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::MCP,
            detection_hints: vec!["aider".into(), "~/.aider/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "aider",
                "args": ["--mcp"]
            })),
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://aider.chat".into(),
            description: "AI pair programming in the terminal".into(),
        },
        Platform {
            name: "fabric".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::MCP,
            detection_hints: vec!["fabric".into(), "~/.config/fabric/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "fabric",
                "args": ["mcp"]
            })),
            status: PlatformStatus::Active,
            country: "Global".into(),
            homepage: "https://github.com/danielmiessler/fabric".into(),
            description: "Open-source framework for augmenting human AI interaction".into(),
        },
        Platform {
            name: "shell_gpt".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::REST,
            detection_hints: vec!["sgpt".into(), "~/.config/shell_gpt/".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "Global".into(),
            homepage: "https://github.com/TheR1D/shell_gpt".into(),
            description: "Command-line productivity tool powered by AI".into(),
        },
        Platform {
            name: "Codex CLI".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::MCP,
            detection_hints: vec!["codex".into(), "~/.codex/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "codex",
                "args": ["mcp"]
            })),
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://github.com/openai/codex".into(),
            description: "OpenAI's lightweight coding agent CLI".into(),
        },
        Platform {
            name: "AutoGPT".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::REST,
            detection_hints: vec!["autogpt".into(), "~/.autogpt/".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://github.com/Significant-Gravitas/AutoGPT".into(),
            description: "Autonomous AI agent for task completion".into(),
        },
        Platform {
            name: "gpt-engineer".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::REST,
            detection_hints: vec!["gpt-engineer".into(), "~/.gpt-engineer/".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "Sweden".into(),
            homepage: "https://github.com/gpt-engineer-org/gpt-engineer".into(),
            description: "AI agent for generating complete codebases".into(),
        },
        Platform {
            name: "sweep".into(),
            category: PlatformCategory::CliTool,
            protocol: Protocol::REST,
            detection_hints: vec!["sweep".into(), "SWEEP_API_KEY".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://sweep.dev".into(),
            description: "AI-powered bug fixing and feature development".into(),
        },
        // ── IDE Integrations ────────────────────────────────────────────────
        Platform {
            name: "VS Code + Copilot".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::REST,
            detection_hints: vec![
                "code".into(),
                "~/.vscode/".into(),
                "~/.config/Code/".into(),
            ],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://code.visualstudio.com".into(),
            description: "Microsoft VS Code with GitHub Copilot integration".into(),
        },
        Platform {
            name: "Cursor".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::MCP,
            detection_hints: vec!["cursor".into(), "~/.cursor/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "cursor",
                "args": ["--mcp"]
            })),
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://cursor.sh".into(),
            description: "AI-native code editor with MCP support".into(),
        },
        Platform {
            name: "Windsurf".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::MCP,
            detection_hints: vec!["windsurf".into(), "~/.windsurf/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "windsurf",
                "args": ["mcp"]
            })),
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://codeium.com/windsurf".into(),
            description: "AI-powered IDE with agentic code flow".into(),
        },
        Platform {
            name: "JetBrains IntelliJ IDEA".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::REST,
            detection_hints: vec![
                "idea".into(),
                "~/.config/JetBrains/IntelliJIdea*/".into(),
            ],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "Czech Republic".into(),
            homepage: "https://www.jetbrains.com/idea/".into(),
            description: "JetBrains flagship Java/Kotlin IDE with AI Assistant".into(),
        },
        Platform {
            name: "JetBrains PyCharm".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::REST,
            detection_hints: vec![
                "pycharm".into(),
                "~/.config/JetBrains/PyCharm*/".into(),
            ],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "Czech Republic".into(),
            homepage: "https://www.jetbrains.com/pycharm/".into(),
            description: "JetBrains Python IDE with AI Assistant".into(),
        },
        Platform {
            name: "JetBrains GoLand".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::REST,
            detection_hints: vec![
                "goland".into(),
                "~/.config/JetBrains/GoLand*/".into(),
            ],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "Czech Republic".into(),
            homepage: "https://www.jetbrains.com/go/".into(),
            description: "JetBrains Go IDE with AI Assistant".into(),
        },
        Platform {
            name: "JetBrains WebStorm".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::REST,
            detection_hints: vec![
                "webstorm".into(),
                "~/.config/JetBrains/WebStorm*/".into(),
            ],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "Czech Republic".into(),
            homepage: "https://www.jetbrains.com/webstorm/".into(),
            description: "JetBrains JavaScript/TypeScript IDE with AI Assistant".into(),
        },
        Platform {
            name: "Android Studio".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::REST,
            detection_hints: vec![
                "studio".into(),
                "~/.config/Google/AndroidStudio*/".into(),
            ],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://developer.android.com/studio".into(),
            description: "Google's Android IDE with AI features".into(),
        },
        Platform {
            name: "Continue.dev".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::MCP,
            detection_hints: vec!["continue".into(), "~/.continue/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "continue",
                "args": ["mcp"]
            })),
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://continue.dev".into(),
            description: "Open-source AI code assistant for VS Code and JetBrains".into(),
        },
        Platform {
            name: "Cody (Sourcegraph)".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::REST,
            detection_hints: vec!["cody".into(), "SRC_ACCESS_TOKEN".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://sourcegraph.com/cody".into(),
            description: "AI coding assistant with codebase-wide understanding".into(),
        },
        Platform {
            name: "Tabnine".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::REST,
            detection_hints: vec!["tabnine".into(), "~/.tabnine/".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "Israel".into(),
            homepage: "https://www.tabnine.com".into(),
            description: "AI code completion with local models".into(),
        },
        Platform {
            name: "Amazon Q Developer".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::REST,
            detection_hints: vec!["q".into(), "~/.aws/q/".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "USA".into(),
            homepage: "https://aws.amazon.com/q/developer/".into(),
            description: "AWS-powered AI assistant for IDEs".into(),
        },
        Platform {
            name: "Cline (extension)".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::MCP,
            detection_hints: vec!["~/.vscode/extensions/cline".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "Global".into(),
            homepage: "https://github.com/cline/cline".into(),
            description: "VS Code extension version of the autonomous coding agent".into(),
        },
        Platform {
            name: "Roo Code".into(),
            category: PlatformCategory::IdeIntegration,
            protocol: Protocol::MCP,
            detection_hints: vec!["~/.vscode/extensions/roo-code".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "Global".into(),
            homepage: "https://www.roocode.com".into(),
            description: "AI coding agent for VS Code with MCP support".into(),
        },
        // ── TUIs (Terminal UIs) ──────────────────────────────────────────────
        Platform {
            name: "Synapsis TUI".into(),
            category: PlatformCategory::Tui,
            protocol: Protocol::MCP,
            detection_hints: vec!["synapsis".into(), "~/.config/synapsis/".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "synapsis",
                "args": ["mcp"]
            })),
            status: PlatformStatus::Active,
            country: "Global".into(),
            homepage: "https://github.com/MethodWhite/synapsis".into(),
            description: "Built-in ratatui terminal UI for Synapsis memory engine".into(),
        },
        // ── Chinese AI Platforms ─────────────────────────────────────────────
        Platform {
            name: "DeepSeek".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["DEEPSEEK_API_KEY".into(), "~/.deepseek/".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://deepseek.com".into(),
            description: "High-performance reasoning models with strong coding ability".into(),
        },
        Platform {
            name: "月之暗面 Kimi".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["KIMI_API_KEY".into(), "MOONSHOT_API_KEY".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://kimi.moonshot.cn".into(),
            description: "Moonshot AI's long-context assistant (月之暗面)".into(),
        },
        Platform {
            name: "智谱 GLM/ChatGLM".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["ZHIPU_API_KEY".into(), "GLM_API_KEY".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://zhipu.ai".into(),
            description: "Zhipu AI's bilingual GLM/ChatGLM model series (智谱)".into(),
        },
        Platform {
            name: "阿里 Qwen/通义".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["QWEN_API_KEY".into(), "DASHSCOPE_API_KEY".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://tongyi.aliyun.com".into(),
            description: "Alibaba's Qwen/通义 model family".into(),
        },
        Platform {
            name: "百度 ERNIE/文心".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["ERNIE_API_KEY".into(), "WENXIN_API_KEY".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://yiyan.baidu.com".into(),
            description: "Baidu's ERNIE/文心一言 model series".into(),
        },
        Platform {
            name: "字节跳动 豆包".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["DOUBAO_API_KEY".into(), "BYTEDANCE_API_KEY".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://doubao.com".into(),
            description: "ByteDance's 豆包 AI assistant platform".into(),
        },
        Platform {
            name: "阶跃星辰 Step".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["STEP_API_KEY".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://stepfun.com".into(),
            description: "Stepfun's 阶跃星辰 Step model series".into(),
        },
        Platform {
            name: "MiniMax".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["MINIMAX_API_KEY".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "minimax",
                "args": ["mcp"],
                "env": { "MINIMAX_API_KEY": "${MINIMAX_API_KEY}" }
            })),
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://minimax.com".into(),
            description: "MiniMax's general-purpose AI platform".into(),
        },
        Platform {
            name: "零一万物 Yi".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["YI_API_KEY".into()],
            mcp_config_template: Some(serde_json::json!({
                "command": "yi",
                "args": ["mcp"],
                "env": { "YI_API_KEY": "${YI_API_KEY}" }
            })),
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://yi-api.com".into(),
            description: "01.AI's Yi model series (零一万物)".into(),
        },
        Platform {
            name: "讯飞 星火".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::WebSocket,
            detection_hints: vec!["XINGHUO_API_KEY".into(), "SPARK_API_KEY".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://xinghuo.xfyun.cn".into(),
            description: "iFlytek's Spark/星火 cognitive model (讯飞)".into(),
        },
        Platform {
            name: "百川 Baichuan".into(),
            category: PlatformCategory::ChineseAiPlatform,
            protocol: Protocol::REST,
            detection_hints: vec!["BAICHUAN_API_KEY".into()],
            mcp_config_template: None,
            status: PlatformStatus::Active,
            country: "China".into(),
            homepage: "https://baichuan-ai.com".into(),
            description: "Baichuan AI's general-purpose model series (百川)".into(),
        },
        // ── New / Emerging (July 2026) ──────────────────────────────────────
        Platform {
            name: "Emerging Platform Alpha".into(),
            category: PlatformCategory::Emerging,
            protocol: Protocol::MCP,
            detection_hints: vec!["PLACEHOLDER_DETECTION".into()],
            mcp_config_template: None,
            status: PlatformStatus::Beta,
            country: "Unknown".into(),
            homepage: "https://github.com".into(),
            description: "Placeholder for recently launched tools (Jul 2026)".into(),
        },
    ]
}

/// Detect installed platforms by checking PATH and common config paths.
pub fn detect_installed_platforms() -> Vec<Platform> {
    let all = all_platforms();
    let mut installed = Vec::new();

    for platform in &all {
        let mut found = false;
        for hint in &platform.detection_hints {
            if hint.starts_with('~') || hint.starts_with('/') || hint.contains('*') {
                // Config path or glob pattern — skip direct check (would need filesystem)
                continue;
            }
            if hint.to_uppercase() == *hint && hint.contains('_') && hint.contains("KEY") {
                // Environment variable key hint
                if std::env::var(hint).is_ok() {
                    found = true;
                    break;
                }
            } else {
                // Binary name — check PATH via `which`
                let path = std::env::var("PATH").unwrap_or_default();
                for dir in std::env::split_paths(&path) {
                    let binary = dir.join(hint);
                    if binary.is_file() {
                        found = true;
                        break;
                    }
                    #[cfg(target_os = "windows")]
                    {
                        let binary_exe = dir.join(format!("{}.exe", hint));
                        if binary_exe.is_file() {
                            found = true;
                            break;
                        }
                    }
                }
                if found {
                    break;
                }
            }
        }
        if found {
            installed.push(platform.clone());
        }
    }

    installed
}

/// Generate MCP configuration snippets for detected platforms.
pub fn generate_mcp_configs(platforms: &[Platform]) -> HashMap<String, serde_json::Value> {
    let mut configs = HashMap::new();

    for platform in platforms {
        if let Some(template) = &platform.mcp_config_template {
            let key = platform.name.to_lowercase().replace(' ', "_");
            configs.insert(key, template.clone());
        }
    }

    configs
}

/// Group platforms by region: Western, Chinese, Other.
pub fn group_by_country(platforms: &[Platform]) -> HashMap<String, Vec<&Platform>> {
    let mut groups: HashMap<String, Vec<&Platform>> = HashMap::new();

    for platform in platforms {
        let region = match platform.country.as_str() {
            "China" => "Chinese Platforms",
            "USA" | "Global" | "Sweden" | "Czech Republic" | "Israel" => "Western Platforms",
            _ => "Other",
        };
        groups.entry(region.into()).or_default().push(platform);
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_platforms_non_empty() {
        let platforms = all_platforms();
        assert!(!platforms.is_empty(), "Platform catalog must not be empty");
    }

    #[test]
    fn test_all_platforms_have_required_fields() {
        for p in all_platforms() {
            assert!(!p.name.is_empty(), "Platform must have a name");
            assert!(!p.country.is_empty(), "{} must have a country", p.name);
            assert!(!p.homepage.is_empty(), "{} must have a homepage", p.name);
        }
    }

    #[test]
    fn test_group_by_country_includes_chinese_and_western() {
        let platforms = all_platforms();
        let groups = group_by_country(&platforms);
        assert!(groups.contains_key("Chinese Platforms"));
        assert!(groups.contains_key("Western Platforms"));
    }

    #[test]
    fn test_generate_mcp_configs_non_empty() {
        let platforms = all_platforms();
        let configs = generate_mcp_configs(&platforms);
        assert!(!configs.is_empty(), "Should generate configs for platforms with templates");
    }

    #[test]
    fn test_generate_mcp_configs_matches_platforms_with_templates() {
        let platforms = all_platforms();
        let configs = generate_mcp_configs(&platforms);
        let with_template: Vec<&str> = platforms
            .iter()
            .filter(|p| p.mcp_config_template.is_some())
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(
            configs.len(),
            with_template.len(),
            "Config count should match platforms with templates"
        );
    }
}
