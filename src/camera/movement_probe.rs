//! GAT-based abstraction for movement detection.
//!
//! This module provides a unified abstraction over blocking and async
//! movement detection, eliminating code duplication.

use std::time::Duration;

use crate::error::Error;

/// Position data for movement detection.
#[derive(Debug, Clone, Copy)]
pub struct PanTiltPosition {
    /// Pan position in camera units.
    pub pan: i16,
    /// Tilt position in camera units.
    pub tilt: i16,
}

/// Movement detection probe that abstracts over blocking/async.
pub trait MovementProbe {
    /// The future type returned by sleep.
    /// For blocking code, this can be a ready future.
    /// For async code, this is the actual sleep future.
    type Sleep<'a>: core::future::Future<Output = ()> + 'a
    where
        Self: 'a;

    /// The future type for getting position.
    type PositionFuture<'a>: core::future::Future<Output = Result<PanTiltPosition, Error>> + 'a
    where
        Self: 'a;

    /// Get the current position.
    fn get_position(&self) -> Self::PositionFuture<'_>;

    /// Sleep for the specified duration.
    fn sleep(&self, duration: Duration) -> Self::Sleep<'_>;
}

/// Zoom movement probe.
pub trait ZoomProbe {
    /// Sleep future type.
    type Sleep<'a>: core::future::Future<Output = ()> + 'a
    where
        Self: 'a;

    /// Zoom query future type.
    type ZoomFuture<'a>: core::future::Future<Output = Result<u16, Error>> + 'a
    where
        Self: 'a;

    /// Get the current zoom position.
    fn get_zoom(&self) -> Self::ZoomFuture<'_>;
    /// Sleep for the specified duration.
    fn sleep(&self, duration: Duration) -> Self::Sleep<'_>;
}

/// Focus movement probe.
pub trait FocusProbe {
    /// Sleep future type.
    type Sleep<'a>: core::future::Future<Output = ()> + 'a
    where
        Self: 'a;

    /// Focus query future type.
    type FocusFuture<'a>: core::future::Future<Output = Result<u16, Error>> + 'a
    where
        Self: 'a;

    /// Get the current focus position.
    fn get_focus(&self) -> Self::FocusFuture<'_>;
    /// Sleep for the specified duration.
    fn sleep(&self, duration: Duration) -> Self::Sleep<'_>;
}

/// Configuration for movement detection.
#[derive(Debug, Clone, Copy)]
pub struct MovementDetectionConfig {
    /// Maximum time to wait for completion.
    pub timeout: Duration,
    /// How often to check position.
    pub poll_interval: Duration,
    /// Initial delay before checking.
    pub startup_delay: Duration,
    /// How many consecutive stable readings needed.
    pub stability_threshold: usize,
    /// Enable debug logging.
    pub debug: bool,
    /// Pan tolerance for detecting movement start (in camera units).
    pub tolerance_pan_start: i16,
    /// Tilt tolerance for detecting movement start (in camera units).
    pub tolerance_tilt_start: i16,
    /// Pan tolerance for detecting stable position (in camera units).
    pub tolerance_pan_stable: i16,
    /// Tilt tolerance for detecting stable position (in camera units).
    pub tolerance_tilt_stable: i16,
    /// Pan/tilt tolerance for oscillation detection (in camera units).
    pub tolerance_pan_tilt_oscillation: i16,
    /// Zoom tolerance for detecting movement start.
    pub tolerance_zoom_start: u16,
    /// Zoom tolerance for detecting stable position.
    pub tolerance_zoom_stable: u16,
    /// Focus tolerance for detecting movement start.
    pub tolerance_focus_start: u16,
    /// Focus tolerance for detecting stable position.
    pub tolerance_focus_stable: u16,
    /// Maximum oscillation samples to track.
    pub oscillation_sample_size: usize,
    /// Minimum oscillation samples needed for detection.
    pub oscillation_min_samples: usize,
    /// Time to wait before assuming no movement will occur.
    pub no_movement_timeout: Duration,
}

impl Default for MovementDetectionConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            poll_interval: Duration::from_millis(100),
            startup_delay: Duration::from_millis(200),
            stability_threshold: 3,
            debug: false,
            tolerance_pan_start: 5,
            tolerance_tilt_start: 5,
            tolerance_pan_stable: 2,
            tolerance_tilt_stable: 2,
            tolerance_pan_tilt_oscillation: 10,
            tolerance_zoom_start: 20,
            tolerance_zoom_stable: 10,
            tolerance_focus_start: 10,
            tolerance_focus_stable: 5,
            oscillation_sample_size: 10,
            oscillation_min_samples: 6,
            no_movement_timeout: Duration::from_secs(2),
        }
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
