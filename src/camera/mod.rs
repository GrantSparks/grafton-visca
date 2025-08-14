//! Camera module providing compile-time profile-based APIs.
//!
//! This module provides the generic `Camera<M, P, T, E>` API with compile-time mode and profile selection
//! for zero runtime overhead. All mode and profile-specific behavior is resolved at compile time.
//!
//! The new executor-based API prevents runtime/spawner mismatches by ensuring all async operations
//! use the same executor type.
//!
//! Use the type aliases for cleaner syntax:
//! - `CameraAsync<P, T>` for async cameras (requires executor)
//! - `CameraBlocking<P, T>` for blocking cameras

pub mod builder_executor;
pub mod capability_introspection;
pub mod generic_executor;
pub mod methods;
pub mod mode;
pub mod movement_detection;
pub mod movement_probe;
pub mod profiles;

// Re-export the new executor-based Camera as the primary Camera type
pub use generic_executor::Camera;

// Re-export mode markers and type aliases
pub use mode::{AsyncMode, BlockingMode, CameraAsync, CameraBlocking, CameraMode};

// Re-export builder types
pub use builder_executor::CameraBuilder;

// Re-export movement detection types
pub use movement_probe::{MovementConfig, PanTiltPosition};

// MovementOps traits have been removed - movement methods are now inherent methods on Camera
