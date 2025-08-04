//! Movement detection utilities and configuration.
//!
//! This module provides a clean event-driven movement detection API that uses
//! VISCA completion messages when available and falls back to efficient state
//! querying when needed.

use std::time::Duration;

/// Position data for movement detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltPosition {
    /// Pan position in camera units.
    pub pan: i16,
    /// Tilt position in camera units.
    pub tilt: i16,
}

/// Configuration for movement detection.
///
/// This configuration is used for both event-driven detection (using VISCA
/// completion messages) and fallback state-query detection.
#[derive(Debug, Clone, Copy)]
pub struct MovementConfig {
    /// Maximum time to wait for movement to complete.
    /// Default: 30 seconds.
    pub timeout: Duration,

    /// Enable debug logging for movement detection.
    /// Default: false.
    pub debug: bool,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            debug: false,
        }
    }
}

impl MovementConfig {
    /// Create a new configuration with a specific timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            ..Default::default()
        }
    }

    /// Enable debug logging.
    pub fn with_debug(mut self) -> Self {
        self.debug = true;
        self
    }
}

/// Check if two positions are equal within tolerance.
#[inline]
pub fn positions_equal_within_tolerance(
    pos1: PanTiltPosition,
    pos2: PanTiltPosition,
    tolerance: i16,
) -> bool {
    (pos1.pan - pos2.pan).abs() <= tolerance && (pos1.tilt - pos2.tilt).abs() <= tolerance
}

/// Check if two positions are equal within separate pan/tilt tolerances.
#[inline]
pub fn positions_equal_within_tolerance_separate(
    pos1: PanTiltPosition,
    pos2: PanTiltPosition,
    pan_tolerance: i16,
    tilt_tolerance: i16,
) -> bool {
    (pos1.pan - pos2.pan).abs() <= pan_tolerance && (pos1.tilt - pos2.tilt).abs() <= tilt_tolerance
}

/// Check if two zoom values are equal within tolerance.
#[inline]
pub fn zoom_equal_within_tolerance(z1: u16, z2: u16, tolerance: u16) -> bool {
    (z1 as i32 - z2 as i32).abs() <= tolerance as i32
}
