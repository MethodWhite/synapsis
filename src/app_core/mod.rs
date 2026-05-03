//! Synapsis Core Application Logic
//!
//! Internal modules for security, resilience, and system management.

pub mod anti_tampering;
pub mod audit_logger;
pub mod resilience;
pub mod self_healing;
pub mod updater;

// Re-export the digital signature locally
pub mod dilithium {
    include!("dilithium_signature.rs");
}
