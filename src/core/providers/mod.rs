//! LLM Provider Implementations

pub mod ollama;

use std::sync::Arc;
use std::collections::HashMap;
use crate::domain::provider::LlmProvider;

/// Registry of available LLM providers
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
    default_provider: Option<String>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            default_provider: None,
        }
    }

    pub fn register(&mut self, provider: Arc<dyn LlmProvider>) {
        let id = provider.id().to_string();
        if self.default_provider.is_none() {
            self.default_provider = Some(id.clone());
        }
        self.providers.insert(id, provider);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn LlmProvider>> {
        self.providers.get(id).cloned()
    }

    pub fn get_default(&self) -> Option<Arc<dyn LlmProvider>> {
        self.default_provider.as_ref().and_then(|id| self.get(id))
    }

    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
