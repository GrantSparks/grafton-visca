//! Testing utilities for the grafton-visca library
//!
//! This module provides testing infrastructure, including a VISCA camera simulator
//! that accurately models the protocol behavior for integration testing.

#[cfg(feature = "rt-tokio")]
pub mod camera_simulator;

#[cfg(feature = "rt-tokio")]
pub use camera_simulator::ViscaCameraSimulator;

/// Testing toolkit for deterministic and scriptable transport testing.
///
/// This module provides utilities for writing deterministic tests that don't rely
/// on real wall-clock time or unpredictable timing behavior.
#[cfg(any(test, feature = "test-utils"))]
pub mod testkit;
