//! Unified camera module with Send-safe, profile-centric VISCA API.
//!
//! This module provides a single, unified camera implementation that works in both
//! async and blocking modes through the Mode trait system. All mode-specific behavior
//! is resolved at compile time for zero runtime overhead.
//!
//! The unified design ensures Send-safe futures and eliminates the complexity of
//! separate AsyncCamera/BlockingCamera types.
//!
//! Use the type aliases for cleaner syntax:
//! - `AsyncCamera<P, Tr, Exec>` for async cameras with runtime-specific executors
//! - `BlockingCamera<P, Tr>` for blocking cameras

pub mod builder;
pub mod capabilities;
pub mod controls;
pub mod movement_detection;
pub mod movement_probe;
pub mod profiles;
pub mod unified;

// Re-export the unified camera types with convenient aliases
pub use unified::Camera;

// Type aliases for easier usage
/// Async camera type alias for easier usage.
///
/// This type represents a camera operating in async mode with Send-safe futures.
/// It requires an async transport and executor for operation.
#[cfg(feature = "async")]
pub type AsyncCamera<P, Tr, Exec> = Camera<crate::mode::Async, P, Tr, Exec>;

/// Blocking camera type alias for easier usage.
///
/// This type represents a camera operating in blocking mode with synchronous operations.
/// It requires a sync transport for operation.
#[cfg(not(feature = "async"))]
pub type BlockingCamera<P, Tr> = Camera<crate::mode::Blocking, P, Tr, ()>;

// Re-export builder types
pub use builder::CameraBuilder;

// Re-export camera type aliases for convenience
#[cfg(all(feature = "async", feature = "rt-tokio"))]
pub use builder::async_cameras::TokioCamera;

#[cfg(all(feature = "async", feature = "rt-async-std"))]
pub use builder::async_std_cameras::AsyncStdCamera;

#[cfg(all(feature = "async", feature = "rt-smol"))]
pub use builder::smol_cameras::SmolCamera;

// Re-export movement detection types
pub use movement_probe::{MovementConfig, PanTiltPosition};
