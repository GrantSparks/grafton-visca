//! Camera module providing compile-time profile-based APIs.
//!
//! This module provides the generic `Camera<P, T>` API with compile-time profile selection
//! for zero runtime overhead. All profile-specific behavior is resolved at compile time.
//!
//! Use the type aliases in the prelude for cleaner syntax:
//! - `PTZOpticsG2Cam<T>` for PTZOptics G2 cameras
//! - `SonyFR7Cam<T>` for Sony FR7 cameras
//! - etc.

pub mod builder;
pub mod generic;
pub mod generic_methods;
pub mod generic_state;
pub mod helpers;
pub mod methods;
pub mod movement_detection;
pub mod movement_probe;
pub mod profiles;

// Re-export the generic Camera as the primary Camera type
pub use generic::Camera;

// Re-export state management
pub use generic_state::CameraState;

// Re-export builder types
pub use builder::CameraBuilder;

#[cfg(not(feature = "async"))]
pub use builder::{TcpBuilder, TypedTcpBuilder, TypedUdpBuilder, UdpBuilder};

#[cfg(feature = "tokio")]
pub use builder::{TokioTcpBuilder, TokioUdpBuilder, TypedTokioTcpBuilder, TypedTokioUdpBuilder};

// Re-export movement detection types
pub use movement_probe::{MovementConfig, PanTiltPosition};

// Re-export helper traits
pub use helpers::MovementHelpers;
#[cfg(feature = "async")]
pub use helpers::MovementHelpersAsync;
