use crate::core::platform_catalog::{all_platforms, detect_installed_platforms};
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct McpConfigEntry {
    pub platform_name: String,
    pub config_path: String,
    pub config_content: serde_json::Value,
    pub installed: bool,
}

#[derive(Debug, Serialize)]
pub struct AutoConfigReport {
    pub generated: Vec<McpConfigEntry>,
    pub skipped: Vec<String>,
}

pub fn detect_and_generate_configs() -> AutoConfigReport {
    let platforms = detect_installed_platforms();
    let mut generated = Vec::new();
    let mut skipped = Vec::new();

    for platform in &platforms {
        match get_config_target_path(&platform.name) {
            Some(config_path) => {
                let entry = build_entry(&platform.name, &config_path);
                generated.push(entry);
            }
            None => {
                skipped.push(platform.name.clone());
            }
        }
    }

    let all = all_platforms();
    if all.iter().any(|p| p.name == "OpenCode") {
        let synapsis_entry = generate_synapsis_mcp_entry();
        if !generated
            .iter()
            .any(|e| e.config_path == synapsis_entry.config_path)
        {
            generated.push(synapsis_entry);
        }
    }

    AutoConfigReport { generated, skipped }
}

pub fn write_configs(report: &AutoConfigReport, dry_run: bool) -> Result<()> {
    for entry in &report.generated {
        let path = PathBuf::from(&entry.config_path);

        if dry_run {
            println!("[DRY RUN] {} -> {}", entry.platform_name, path.display());
            println!("  {}", serde_json::to_string_pretty(&entry.config_content)?);
            continue;
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let final_content = merge_with_existing(&path, &entry.config_content)
            .with_context(|| format!("Failed to merge config for {}", path.display()))?;

        std::fs::write(&path, &final_content)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;

        println!("  Wrote config to {}", path.display());
    }

    Ok(())
}

pub fn get_config_target_path(platform_name: &str) -> Option<String> {
    let home = dirs::home_dir()?;

    match platform_name {
        "OpenCode" => Some(
            home.join(".config/opencode/opencode.jsonc")
                .to_string_lossy()
                .to_string(),
        ),
        "Claude Code" => Some(
            home.join(".config/claude/mcp.json")
                .to_string_lossy()
                .to_string(),
        ),
        "Cursor" => Some(home.join(".cursor/mcp.json").to_string_lossy().to_string()),
        "Windsurf" => Some(
            home.join(".windsurf/mcp.json")
                .to_string_lossy()
                .to_string(),
        ),
        "Gemini CLI" => Some(
            home.join(".config/gemini/mcp.json")
                .to_string_lossy()
                .to_string(),
        ),
        "Cline" => Some(
            home.join(".config/cline/mcp.json")
                .to_string_lossy()
                .to_string(),
        ),
        "Continue.dev" => Some(
            home.join(".continue/config.json")
                .to_string_lossy()
                .to_string(),
        ),
        "VS Code + Copilot" => Some(home.join(".vscode/mcp.json").to_string_lossy().to_string()),
        name if name.starts_with("JetBrains") => None,
        "Synapsis TUI" => Some(
            home.join(".config/synapsis/mcp.json")
                .to_string_lossy()
                .to_string(),
        ),
        _ => None,
    }
}

pub fn generate_synapsis_mcp_entry() -> McpConfigEntry {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let config_path = home
        .join(".config/opencode/opencode.jsonc")
        .to_string_lossy()
        .to_string();

    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("synapsis-mcp"))
        .to_string_lossy()
        .to_string();

    let config_content = serde_json::json!({
        "mcp": {
            "synapsis": {
                "command": exe_path,
                "args": []
            }
        }
    });

    McpConfigEntry {
        platform_name: "OpenCode".into(),
        config_path,
        config_content,
        installed: true,
    }
}

fn build_entry(platform_name: &str, config_path: &str) -> McpConfigEntry {
    let config_content = wrap_synapsis_entry(platform_name);
    McpConfigEntry {
        platform_name: platform_name.to_string(),
        config_path: config_path.to_string(),
        config_content,
        installed: true,
    }
}

fn wrap_synapsis_entry(platform_name: &str) -> serde_json::Value {
    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("synapsis-mcp"))
        .to_string_lossy()
        .to_string();

    let synapsis_server = serde_json::json!({
        "command": exe_path,
        "args": []
    });

    match platform_name {
        "OpenCode" => serde_json::json!({ "mcp": { "synapsis": synapsis_server } }),
        "Cursor" | "Windsurf" | "Gemini CLI" | "Cline" | "VS Code + Copilot"
        | "Claude Code" | "Continue.dev" | "Synapsis TUI" => serde_json::json!({
            "mcpServers": { "synapsis": synapsis_server }
        }),
        _ => synapsis_server,
    }
}

fn merge_with_existing(path: &std::path::Path, new_content: &serde_json::Value) -> Result<String> {
    if !path.exists() {
        return Ok(serde_json::to_string_pretty(new_content)?);
    }

    let existing = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read existing config: {}", path.display()))?;

    let existing_json: serde_json::Value =
        serde_json::from_str(&existing).unwrap_or(serde_json::Value::Null);

    let merged = if let (Some(existing_obj), Some(new_obj)) =
        (existing_json.as_object(), new_content.as_object())
    {
        let mut merged = existing_obj.clone();
        for (key, value) in new_obj {
            if key == "mcpServers" || key == "mcp" {
                // Merge into the correct section depending on platform
                if let Some(new_map) = value.as_object() {
                    let mut target = merged.get(key)
                        .and_then(|v| v.as_object().cloned())
                        .unwrap_or_default();
                    for (k, v) in new_map {
                        target.insert(k.clone(), v.clone());
                    }
                    merged.insert(key.clone(), serde_json::Value::Object(target));
                } else {
                    merged.insert(key.clone(), value.clone());
                }
            } else if let (Some(existing_val), Some(new_map)) = (merged.get(key), value.as_object()) {
                if let Some(existing_map) = existing_val.as_object() {
                    let mut server_map = existing_map.clone();
                    for (k, v) in new_map {
                        server_map.insert(k.clone(), v.clone());
                    }
                    merged.insert(key.clone(), serde_json::Value::Object(server_map));
                } else {
                    merged.insert(key.clone(), value.clone());
                }
            } else {
                merged.insert(key.clone(), value.clone());
            }
        }
        serde_json::Value::Object(merged)
    } else {
        new_content.clone()
    };

    Ok(serde_json::to_string_pretty(&merged)?)
}
