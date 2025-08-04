//! GAT-based abstraction for movement detection.
//!
//! This module provides a unified abstraction over blocking and async
//! movement detection, eliminating code duplication.

use std::time::Duration;

use crate::error::Error;

/// Position data for movement detection.
#[derive(Debug, Clone, Copy)]
pub struct PanTiltPosition {
    pub pan: i16,
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

    /// Get the current pan/tilt position.
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
    type Sleep<'a>: core::future::Future<Output = ()> + 'a
    where
        Self: 'a;

    type ZoomFuture<'a>: core::future::Future<Output = Result<u16, Error>> + 'a
    where
        Self: 'a;

    fn get_zoom(&self) -> Self::ZoomFuture<'_>;
    fn sleep(&self, duration: Duration) -> Self::Sleep<'_>;
}

/// Focus movement probe.
pub trait FocusProbe {
    type Sleep<'a>: core::future::Future<Output = ()> + 'a
    where
        Self: 'a;

    type FocusFuture<'a>: core::future::Future<Output = Result<u16, Error>> + 'a
    where
        Self: 'a;

    fn get_focus(&self) -> Self::FocusFuture<'_>;
    fn sleep(&self, duration: Duration) -> Self::Sleep<'_>;
}

/// Configuration for movement detection.
#[derive(Debug, Clone)]
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
}

impl Default for MovementDetectionConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            poll_interval: Duration::from_millis(100),
            startup_delay: Duration::from_millis(200),
            stability_threshold: 3,
            debug: false,
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

/// Check if two zoom values are equal within tolerance.
#[inline]
pub fn zoom_equal_within_tolerance(z1: u16, z2: u16, tolerance: u16) -> bool {
    (z1 as i32 - z2 as i32).abs() <= tolerance as i32
}

