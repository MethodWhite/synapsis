//! Ollama Provider Implementation

use crate::domain::provider::LlmProvider;
use crate::domain::Result;
use std::process::Command;

/// Ollama Model Provider
pub struct OllamaProvider {
    default_model: String,
}

impl OllamaProvider {
    pub fn new(model: Option<&str>) -> Self {
        Self {
            default_model: model.unwrap_or("llama3.2:1b").to_string(),
        }
    }
}

impl LlmProvider for OllamaProvider {
    fn id(&self) -> &str {
        "ollama"
    }

    fn name(&self) -> &str {
        "Ollama Local AI"
    }

    fn is_available(&self) -> bool {
        Command::new("ollama")
            .arg("list")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn list_models(&self) -> Result<Vec<String>> {
        let output = Command::new("ollama")
            .arg("list")
            .output()?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let models: Vec<String> = stdout
                .lines()
                .skip(1) // Skip header
                .filter_map(|line| line.split_whitespace().next().map(String::from))
                .collect();
            Ok(models)
        } else {
            Err(crate::domain::SynapsisError::new(
                crate::domain::ErrorKind::Internal,
                500,
                "Failed to list Ollama models"
            ))
        }
    }

    fn generate(&self, prompt: &str, model: Option<&str>) -> Result<String> {
        let model = model.unwrap_or(&self.default_model);
        
        let output = Command::new("ollama")
            .args(["run", model, prompt])
            .output()?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(crate::domain::SynapsisError::new(
                crate::domain::ErrorKind::Internal,
                500,
                format!("Ollama error: {}", String::from_utf8_lossy(&output.stderr))
            ))
        }
    }
}
