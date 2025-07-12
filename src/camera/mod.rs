//! New camera API with compile-time safety.
//!
//! This module implements the new Camera API where methods only exist
//! for cameras that support the corresponding capabilities.

// Core camera implementation (internal)
pub(crate) mod core;

// Facade implementations (internal)
pub(crate) mod async_facade;
pub(crate) mod blocking_facade;

// Unified camera implementation
pub mod unified;

pub mod methods;
pub mod profiles;

// Re-export only the unified camera types
pub use unified::{ProfileId, UnifiedCamera};
