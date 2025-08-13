//! Camera module providing compile-time profile-based APIs.
//!
//! This module provides the generic `Camera<M, P, T>` API with compile-time mode and profile selection
//! for zero runtime overhead. All mode and profile-specific behavior is resolved at compile time.
//!
//! Use the type aliases for cleaner syntax:
//! - `CameraAsync<P, T>` for async cameras
//! - `CameraBlocking<P, T>` for blocking cameras

pub mod builder;
pub mod capability_introspection;
pub mod generic;
pub mod generic_state;
pub mod helpers;
pub mod methods;
pub mod mode;
pub mod movement_detection;
pub mod movement_probe;
pub mod profiles;

// Re-export the generic Camera as the primary Camera type
pub use generic::Camera;

// Re-export mode markers and type aliases
pub use mode::{AsyncMode, BlockingMode, CameraAsync, CameraBlocking, CameraMode};

// Re-export state management
pub use generic_state::CameraState;

// Re-export builder types
pub use builder::{CameraBuilder, Protocol};

// Re-export movement detection types
pub use movement_probe::{MovementConfig, PanTiltPosition};

// Re-export helper traits
#[cfg(feature = "async")]
pub use helpers::MovementOps;
#[cfg(not(feature = "async"))]
pub use helpers::MovementOpsBlocking;
