//! Model Provider Abstractions
//!
//! Defines the core traits for Large Language Model providers.

use serde::{Deserialize, Serialize};
use crate::domain::Result;

/// Generic response from a model provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub model: String,
    pub content: String,
    pub provider: String,
    pub done: bool,
}

/// Information about a model provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_available: bool,
    pub capabilities: Vec<String>,
}

/// Core trait for LLM providers
pub trait LlmProvider: Send + Sync {
    /// Unique identifier for this provider (e.g., "ollama", "openai")
    fn id(&self) -> &str;
    
    /// User-friendly name for this provider
    fn name(&self) -> &str;
    
    /// Check if the provider is currently available on the system
    fn is_available(&self) -> bool;
    
    /// List models available through this provider
    fn list_models(&self) -> Result<Vec<String>>;
    
    /// Generate a response for a given prompt
    fn generate(&self, prompt: &str, model: Option<&str>) -> Result<String>;
    
    /// Summarize a given text
    fn summarize(&self, text: &str, model: Option<&str>) -> Result<String> {
        let prompt = format!("Summarize this concisely in 3 sentences:\n\n{}", text);
        self.generate(&prompt, model)
    }
}
