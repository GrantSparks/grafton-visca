//! New camera API with compile-time safety.
//!
//! This module implements the new Camera API where methods only exist
//! for cameras that support the corresponding capabilities.

// Core camera implementation
pub mod core;

// Facade implementations  
pub mod async_facade;
pub mod blocking_facade;

pub mod methods;
pub mod profiles;

// Re-export the main camera types
pub use async_facade::CameraAsync;
pub use blocking_facade::CameraBlocking;
pub use core::CameraCore;