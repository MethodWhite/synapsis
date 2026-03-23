//! Synapsis Domain Module
//!
//! Módulo raíz del dominio.

pub mod entities;
pub mod errors;
pub mod ports;
pub mod provider;
pub mod types;

pub use entities::*;
pub use errors::{ErrorKind, Result, SynapsisError};
pub use ports::*;
pub use types::*;
