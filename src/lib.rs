//! Synapsis - Persistent Memory Engine for AI Agents
//!
//! This is the main application crate, building on top of `synapsis-core`
//! to provide the full Synapsis experience with MCP, HTTP, CLI, and TUI interfaces.
#![allow(dead_code, unused_imports, unused_variables, unused_mut)]
#![recursion_limit = "512"]

// Re-export synapsis-core as the foundation
pub use synapsis_core::*;

// Presentation layer (MCP, HTTP, CLI, TUI) - specific to synapsis application
pub mod presentation;
pub mod tools;
pub mod api;

// Session cleanup module - automatic session lifecycle management
pub mod session_cleanup;

// Re-export domain types for convenience
pub use domain::*;

// Security modules (re-export from core)
#[cfg(feature = "security")]
pub mod rate_limiter {
    pub use crate::core::rate_limiter::*;
}

#[cfg(feature = "security")]
pub mod audit_log {
    pub use crate::core::audit_log::*;
}

#[cfg(feature = "security")]
pub mod zero_trust {
    pub use crate::core::zero_trust::*;
}
