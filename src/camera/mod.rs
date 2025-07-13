//! New camera API with compile-time safety.
//!
//! This module implements the new Camera API where methods only exist
//! for cameras that support the corresponding capabilities.

// Core camera implementation removed - using unified Camera directly

// Unified camera implementation
pub mod unified;

// Internal trait for camera wrapper access
pub(crate) mod camera_like;

pub mod methods;
pub mod profiles;

// Re-export only the unified camera types
pub use unified::{Camera, ProfileId};
