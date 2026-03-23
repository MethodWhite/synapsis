//! Synapsis Authentication Module
//!
//! Provides intelligent agent authentication, classification, and permission management.
//!
//! # Components
//!
//! - [`permissions`] - Permission system and trust levels
//! - [`classifier`] - Agent classification based on device and connection type
//! - [`tpm`] - TPM and MFA integration
//! - [`challenge`] - Challenge-response authentication

pub mod permissions;
pub mod classifier;
pub mod tpm;
pub mod challenge;

pub use permissions::*;
pub use classifier::*;
pub use tpm::*;
pub use challenge::*;
