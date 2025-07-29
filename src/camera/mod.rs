//! Camera module providing compile-time profile-based APIs.
//!
//! This module provides the generic `Camera<P, T>` API with compile-time profile selection
//! for zero runtime overhead. All profile-specific behavior is resolved at compile time.
//!
//! Use the type aliases in the prelude for cleaner syntax:
//! - `PTZOpticsG2Cam<T>` for PTZOptics G2 cameras
//! - `SonyFR7Cam<T>` for Sony FR7 cameras
//! - etc.

pub mod generic;
pub mod generic_methods;
pub mod methods;
pub mod profiles;

// Re-export the generic Camera as the primary Camera type
pub use generic::Camera;
