//! Synapsis - Persistent Memory Engine for AI Agents
//!
//! This is the main application crate, building on top of `synapsis-core`
//! to provide the full Synapsis experience with MCP, HTTP, CLI, and TUI interfaces.
#![allow(dead_code, unused_imports, unused_variables, unused_mut)]
#![recursion_limit = "512"]

// Re-export synapsis-core as the foundation
pub use synapsis_core::*;

// App-specific core logic (updater, resilience, etc.)
pub mod app_core;

// CLI
pub mod cli;

// Presentation layer (MCP, HTTP, CLI, TUI) - specific to synapsis application
pub mod api;
pub mod presentation;
pub mod tools;

// Plugins - extended capabilities
pub mod plugins;

// Session cleanup module - automatic session lifecycle management
pub mod session_cleanup;

// Re-export domain types for convenience
pub use domain::*;

// Re-export core modules explicitly to resolve path issues
pub mod rate_limiter {
    pub use synapsis_core::core::rate_limiter::*;
}

pub mod audit_log {
    pub use synapsis_core::core::audit_log::*;
}

pub mod zero_trust {
    pub use synapsis_core::core::zero_trust::*;
}

// PQC Digital Signatures with CRYSTALS-Dilithium
pub mod dilithium {
    pub use crate::app_core::dilithium::*;
}
