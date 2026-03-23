//! Synapsis Recycle Module
//!
//! Provides intelligent message recycling with smart categorization.
//!
//! # Components
//!
//! - [`categorizer`] - Smart message categorization
//! - [`bin`] - Recycle bin storage and search

pub mod categorizer;
pub mod bin;

pub use categorizer::*;
pub use bin::*;
