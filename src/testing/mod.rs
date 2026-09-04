//! Testing utilities for the grafton-visca library
//!
//! This module provides testing infrastructure, including a simulator for Standard
//! VISCA framing in integration tests. Its pan/tilt position inquiries always use
//! the standard signed 4+4-nibble reply form; it is not a profile- or
//! codec-selectable simulator and does not model Sony BRC-300's profile-owned
//! 5+4 position codec.

#[cfg(all(feature = "runtime-tokio", any(test, feature = "test-utils")))]
pub mod camera_simulator;

#[cfg(all(feature = "runtime-tokio", any(test, feature = "test-utils")))]
pub use camera_simulator::ViscaCameraSimulator;

/// Testing toolkit for deterministic and scriptable transport testing.
///
/// This module provides utilities for writing deterministic tests that don't rely
/// on real wall-clock time or unpredictable timing behavior.
#[cfg(any(test, feature = "test-utils"))]
pub mod testkit;

/// Internal protocol fuzz entry points.
#[doc(hidden)]
#[cfg(all(feature = "test-utils", feature = "blocking"))]
pub mod fuzz;
