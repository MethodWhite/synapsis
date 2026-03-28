//! Example of using the Synapsis plugin system
//!
//! This example demonstrates:
//! 1. Creating a plugin registry
//! 2. Registering built-in plugins
//! 3. Using extensions from plugins

use std::sync::Arc;
use synapsis::core::{CryptoPlugin, PqcryptoProvider};
use synapsis::domain::crypto::{CryptoProvider, CryptoProviderRegistry, PqcAlgorithm};
use synapsis::domain::plugin::{PluginRegistry, SynapsisPlugin};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create plugin registry
    let mut plugin_registry = PluginRegistry::new();

    // Register built-in cryptography plugin
    let crypto_plugin = Arc::new(CryptoPlugin::new());
    plugin_registry.register_plugin(crypto_plugin.clone())?;

    // Start all plugins
    plugin_registry.start_all()?;

    // Get crypto providers from the plugin system
    let crypto_providers: Vec<Arc<dyn CryptoProvider>> =
        plugin_registry.get_extensions(synapsis::domain::plugin::ExtensionPoint::CryptoProvider);

    println!("Found {} crypto provider(s)", crypto_providers.len());

    for provider in &crypto_providers {
        println!("Provider: {} ({})", provider.name(), provider.id());
        println!("  Description: {}", provider.description());
        println!(
            "  Supported algorithms: {:?}",
            provider.supported_algorithms()
        );

        // Example: Generate random bytes
        match provider.random_bytes(32) {
            Ok(bytes) => println!(
                "  Generated 32 random bytes: {:?}",
                hex::encode(&bytes[..8])
            ),
            Err(e) => println!("  Error generating random bytes: {}", e),
        }
    }

    // Alternatively, use the CryptoProviderRegistry directly
    let mut crypto_registry = CryptoProviderRegistry::new();

    // Register the comprehensive provider
    let comprehensive_provider = Arc::new(PqcryptoProvider::new());
    crypto_registry.register(comprehensive_provider);

    // Find provider for a specific algorithm
    if let Some(provider) = crypto_registry.find_provider_for_algorithm(PqcAlgorithm::Kyber512) {
        println!("\nFound provider for Kyber512: {}", provider.name());

        // In real usage, you would generate keypairs, encrypt, etc.
        // let (pk, sk) = provider.generate_keypair(PqcAlgorithm::Kyber512)?;
    }

    // Stop all plugins
    plugin_registry.stop_all()?;

    Ok(())
}
