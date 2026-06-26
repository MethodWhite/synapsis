//! Synapsis - Persistent Memory Engine for AI Agents

#![recursion_limit = "512"]

pub mod config;
pub mod core;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod tools;

pub use domain::*;

// Security modules
pub use crate::core::audit_log::AuditLog;
pub use crate::core::rate_limiter::RateLimitError;
