//! Synapsis - Persistent Memory Engine for AI Agents
#![allow(dead_code, unused_imports, unused_variables, unused_mut)]

pub mod core;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod tools;

pub use domain::*;

// Security modules
#[cfg(feature = "security")]
pub mod encryption {
    pub use crate::infrastructure::database::encryption::*;
}

#[cfg(feature = "security")]
pub mod rate_limiter {
    pub use crate::core::rate_limiter::*;
}

#[cfg(feature = "security")]
pub mod audit_log {
    pub use crate::core::audit_log::*;
}
