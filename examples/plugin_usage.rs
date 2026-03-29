//! Example of using the Synapsis plugin system
//!
//! This example demonstrates:
//! 1. Creating a plugin registry
//! 2. Registering built-in plugins
//! 3. Using extensions from plugins

use std::sync::Arc;
use synapsis::core::PqcryptoProvider;
use synapsis::domain::crypto::{CryptoProviderRegistry, PqcAlgorithm};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut plugin_registry = synapsis::domain::plugin::PluginRegistry::new();

    let crypto_plugin = Arc::new(synapsis::core::CryptoPlugin::new());
    plugin_registry.register_plugin(crypto_plugin.clone())?;

    plugin_registry.start_all()?;

    let mut crypto_registry = CryptoProviderRegistry::new();

    let comprehensive_provider = Arc::new(PqcryptoProvider::new());
    crypto_registry.register(comprehensive_provider);

    if let Some(provider) = crypto_registry.find_provider_for_algorithm(PqcAlgorithm::Kyber512) {
        println!("Found provider for Kyber512: {}", provider.name());
    }

    plugin_registry.stop_all()?;

    Ok(())
}
