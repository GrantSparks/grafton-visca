//! Movement detection utilities and configuration.
//!
//! This module provides a comprehensive movement detection API that uses
//! VISCA completion messages when available and falls back to efficient state
//! querying when needed. It includes utilities for position comparison,
//! movement configuration, and both event-driven and state-based detection.
//!
//! # Axis-Specific Movement Detection
//!
//! The [`Axes`] flags allow selective monitoring of specific camera axes:
//!
//! ```ignore
//! // Wait only for pan/tilt to complete (ignore zoom/focus)
//! camera.await_axes_idle(Axes::PAN_TILT, Duration::from_secs(10))?;
//!
//! // Wait for all motion axes
//! camera.await_axes_idle(Axes::ALL, Duration::from_secs(30))?;
//! ```
//!
//! # Movement Configuration
//!
//! Use [`AwaitConfig`] for fine-grained control over movement detection:
//!
//! ```ignore
//! let config = AwaitConfig::new(Duration::from_secs(30))
//!     .with_axes(Axes::PAN_TILT | Axes::ZOOM)
//!     .with_debug();
//!
//! camera.await_with_config(&config)?;
//! ```

use std::time::Duration;

use super::Camera;
use crate::{
    capabilities::{Profile, ProfileMetadata},
    command::inquiry::{FocusPositionInquiry, PanTiltPositionInquiry, ZoomPositionInquiry},
    error::Error,
};

#[cfg(feature = "mode-async")]
use crate::{executor::Executor, transport::AsyncTransport};

#[cfg(not(feature = "mode-async"))]
use crate::{mode::BlockingFutureExt, timeout::Deadline, transport::BlockingTransport};

// ============================================================================
// Axes Bitflags
// ============================================================================

/// Bitflags for selecting which camera axes to monitor for movement.
///
/// This allows efficient, targeted movement detection by polling only the
/// relevant axes rather than querying all positions.
///
/// # Examples
///
/// ```
/// use grafton_visca::camera::Axes;
///
/// // Monitor only pan/tilt
/// let axes = Axes::PAN_TILT;
///
/// // Monitor pan/tilt and zoom (skip focus)
/// let axes = Axes::PAN_TILT | Axes::ZOOM;
///
/// // Monitor all axes
/// let axes = Axes::ALL;
///
/// // Check what's included
/// assert!(Axes::ALL.contains(Axes::PAN_TILT));
/// assert!(Axes::ALL.contains(Axes::ZOOM));
/// assert!(Axes::ALL.contains(Axes::FOCUS));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Axes(u8);

impl Axes {
    /// Pan and tilt axes (typically move together).
    pub const PAN_TILT: Self = Self(0b001);

    /// Zoom axis.
    pub const ZOOM: Self = Self(0b010);

    /// Focus axis.
    pub const FOCUS: Self = Self(0b100);

    /// All movement axes (pan/tilt, zoom, and focus).
    pub const ALL: Self = Self(0b111);

    /// No axes (empty set).
    pub const NONE: Self = Self(0b000);

    /// Check if this set contains the specified axes.
    #[inline]
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Check if this set is empty (no axes selected).
    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true if pan/tilt monitoring is enabled.
    #[inline]
    #[must_use]
    pub const fn has_pan_tilt(self) -> bool {
        (self.0 & Self::PAN_TILT.0) != 0
    }

    /// Returns true if zoom monitoring is enabled.
    #[inline]
    #[must_use]
    pub const fn has_zoom(self) -> bool {
        (self.0 & Self::ZOOM.0) != 0
    }

    /// Returns true if focus monitoring is enabled.
    #[inline]
    #[must_use]
    pub const fn has_focus(self) -> bool {
        (self.0 & Self::FOCUS.0) != 0
    }

    /// Count the number of axes in this set.
    #[inline]
    #[must_use]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }
}

impl std::ops::BitOr for Axes {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Axes {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for Axes {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl Default for Axes {
    /// Default is all axes monitored.
    fn default() -> Self {
        Self::ALL
    }
}

impl std::fmt::Display for Axes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return write!(f, "none");
        }

        let mut parts = Vec::new();
        if self.has_pan_tilt() {
            parts.push("pan/tilt");
        }
        if self.has_zoom() {
            parts.push("zoom");
        }
        if self.has_focus() {
            parts.push("focus");
        }

        write!(f, "{}", parts.join(", "))
    }
}

// ============================================================================
// AwaitConfig Builder
// ============================================================================

/// Configuration for awaiting camera movement completion.
///
/// This builder provides fine-grained control over movement detection behavior,
/// including axis selection, timeout configuration, and debug logging.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use grafton_visca::camera::{AwaitConfig, Axes};
///
/// // Basic configuration with timeout
/// let config = AwaitConfig::new(Duration::from_secs(30));
///
/// // Wait for specific axes with debug logging
/// let config = AwaitConfig::new(Duration::from_secs(10))
///     .with_axes(Axes::PAN_TILT | Axes::ZOOM)
///     .with_debug();
///
/// // Preset recall typically needs all axes and longer timeout
/// let config = AwaitConfig::for_preset_recall();
/// ```
#[derive(Debug, Clone, Copy)]
pub struct AwaitConfig {
    /// Maximum time to wait for movement to complete.
    pub timeout: Duration,

    /// Which axes to monitor for movement.
    pub axes: Axes,

    /// Enable debug logging for movement detection.
    pub debug: bool,

    /// Polling interval between position samples.
    /// Default: 100ms, adapts based on elapsed time.
    pub poll_interval: Duration,

    /// Position tolerance for considering movement stopped.
    /// Higher values mean less sensitivity to micro-movements.
    pub tolerance: MovementTolerance,
}

/// Tolerance values for movement detection.
///
/// These determine how much position change is considered "movement"
/// versus noise or settling.
#[derive(Debug, Clone, Copy)]
pub struct MovementTolerance {
    /// Pan/tilt position tolerance in raw VISCA units.
    /// Default: 2 units (approximately 0.14° for most cameras).
    pub pan_tilt: i16,

    /// Zoom position tolerance in raw VISCA units.
    /// Default: 10 units.
    pub zoom: u16,

    /// Focus position tolerance in raw VISCA units.
    /// Default: 5 units.
    pub focus: u16,
}

impl Default for MovementTolerance {
    fn default() -> Self {
        Self {
            pan_tilt: 2,
            zoom: 10,
            focus: 5,
        }
    }
}

impl AwaitConfig {
    /// Create a new configuration with the specified timeout.
    ///
    /// Defaults to monitoring all axes.
    #[must_use]
    pub const fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            axes: Axes::ALL,
            debug: false,
            poll_interval: Duration::from_millis(100),
            tolerance: MovementTolerance {
                pan_tilt: 2,
                zoom: 10,
                focus: 5,
            },
        }
    }

    /// Create configuration suitable for preset recall operations.
    ///
    /// Presets typically move all axes and may take longer to complete.
    /// Uses a 60-second timeout and monitors all axes.
    #[must_use]
    pub const fn for_preset_recall() -> Self {
        Self::new(Duration::from_secs(60)).with_axes(Axes::ALL)
    }

    /// Create configuration suitable for pan/tilt movements only.
    ///
    /// Uses a 30-second timeout and monitors only pan/tilt.
    #[must_use]
    pub const fn for_pan_tilt() -> Self {
        Self::new(Duration::from_secs(30)).with_axes(Axes::PAN_TILT)
    }

    /// Create configuration suitable for zoom movements only.
    ///
    /// Uses a 15-second timeout and monitors only zoom.
    #[must_use]
    pub const fn for_zoom() -> Self {
        Self::new(Duration::from_secs(15)).with_axes(Axes::ZOOM)
    }

    /// Create configuration suitable for focus movements only.
    ///
    /// Uses a 10-second timeout and monitors only focus.
    #[must_use]
    pub const fn for_focus() -> Self {
        Self::new(Duration::from_secs(10)).with_axes(Axes::FOCUS)
    }

    /// Set which axes to monitor for movement.
    #[must_use]
    pub const fn with_axes(mut self, axes: Axes) -> Self {
        self.axes = axes;
        self
    }

    /// Enable debug logging for movement detection.
    #[must_use]
    pub const fn with_debug(mut self) -> Self {
        self.debug = true;
        self
    }

    /// Set the polling interval between position samples.
    #[must_use]
    pub const fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Set movement tolerance values.
    #[must_use]
    pub const fn with_tolerance(mut self, tolerance: MovementTolerance) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Set pan/tilt tolerance specifically.
    #[must_use]
    pub const fn with_pan_tilt_tolerance(mut self, tolerance: i16) -> Self {
        self.tolerance.pan_tilt = tolerance;
        self
    }

    /// Set the timeout duration.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Default for AwaitConfig {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

impl From<Duration> for AwaitConfig {
    /// Create a basic config from just a timeout duration.
    fn from(timeout: Duration) -> Self {
        Self::new(timeout)
    }
}

// ============================================================================
// Position Types
// ============================================================================

/// Position data for movement detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltPosition {
    /// Pan position in camera units.
    pub pan: i16,
    /// Tilt position in camera units.
    pub tilt: i16,
}

impl PanTiltPosition {
    /// Creates a new pan/tilt position.
    pub const fn new(pan: i16, tilt: i16) -> Self {
        Self { pan, tilt }
    }

    /// Get position in degrees.
    ///
    /// Converts the raw VISCA pan/tilt values to degrees using standard conversion:
    /// - Pan: -170° to +170° (mapped from -2448 to +2448)
    /// - Tilt: -30° to +90° (mapped from -432 to +1296)
    ///
    /// # Returns
    /// A tuple of (pan_degrees, tilt_degrees) as f64 values.
    ///
    /// # Examples
    /// ```
    /// # use grafton_visca::camera::PanTiltPosition;
    /// let pos = PanTiltPosition::new(1224, 648);
    /// let (pan_deg, tilt_deg) = pos.as_degrees();
    /// // pan_deg ≈ 85.0, tilt_deg ≈ 45.0
    /// ```
    #[must_use]
    pub fn as_degrees(&self) -> (f64, f64) {
        use crate::types::{PanPosition, TiltPosition};

        let pan_pos = PanPosition::new(self.pan).unwrap_or(PanPosition::CENTER);
        let tilt_pos = TiltPosition::new(self.tilt).unwrap_or(TiltPosition::CENTER);

        (
            f64::from(pan_pos.to_degrees()),
            f64::from(tilt_pos.to_degrees()),
        )
    }

    /// Get raw position values.
    ///
    /// Returns the underlying VISCA raw values for pan and tilt.
    /// This is useful when you need the raw protocol values.
    ///
    /// # Returns
    /// A tuple of (pan, tilt) as i16 values.
    ///
    /// # Examples
    /// ```
    /// # use grafton_visca::camera::PanTiltPosition;
    /// let pos = PanTiltPosition::new(100, -50);
    /// let (pan, tilt) = pos.raw_values();
    /// assert_eq!(pan, 100);
    /// assert_eq!(tilt, -50);
    /// ```
    #[must_use]
    pub const fn raw_values(&self) -> (i16, i16) {
        (self.pan, self.tilt)
    }
}

/// Check if two positions are equal within tolerance.
#[inline]
fn positions_equal_within_tolerance(
    pos1: PanTiltPosition,
    pos2: PanTiltPosition,
    tolerance: i16,
) -> bool {
    (pos1.pan - pos2.pan).abs() <= tolerance && (pos1.tilt - pos2.tilt).abs() <= tolerance
}

/// Check if two zoom values are equal within tolerance.
#[inline]
fn zoom_equal_within_tolerance(z1: u16, z2: u16, tolerance: u16) -> bool {
    (z1 as i32 - z2 as i32).abs() <= tolerance as i32
}

// Blocking mode implementation is only available without async feature
#[cfg(not(feature = "mode-async"))]
impl<P, T> Camera<crate::mode::Blocking, P, T, ()>
where
    P: Profile + ProfileMetadata + Default,
    T: BlockingTransport + crate::transport::HasTransportConfig + 'static,
{
    /// Wait for pan/tilt movement to complete.
    ///
    /// Convenience method that waits for pan and tilt motors to stop moving.
    /// This delegates to [`Self::await_with_config`] with [`AwaitConfig::for_pan_tilt()`]
    /// settings and the specified timeout.
    pub fn await_pan_tilt_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::for_pan_tilt().with_timeout(timeout))
    }

    /// Wait for zoom movement to complete.
    ///
    /// Convenience method that waits for zoom motor to stop moving.
    /// This delegates to [`Self::await_with_config`] with [`AwaitConfig::for_zoom()`]
    /// settings and the specified timeout.
    pub fn await_zoom_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::for_zoom().with_timeout(timeout))
    }

    /// Wait for focus movement to complete.
    ///
    /// Convenience method that waits for focus motor to stop moving.
    /// This delegates to [`Self::await_with_config`] with [`AwaitConfig::for_focus()`]
    /// settings and the specified timeout.
    pub fn await_focus_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::for_focus().with_timeout(timeout))
    }

    /// Wait for all movements to complete.
    ///
    /// Convenience method that waits for all motors (pan/tilt, zoom, focus) to stop.
    ///
    /// For more control over which axes to monitor, use [`Self::await_with_config`] or
    /// [`Self::await_axes_idle`].
    pub fn await_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::new(timeout))
    }

    /// Check if the camera is currently moving.
    ///
    /// This checks pan/tilt, zoom, and focus positions to detect movement.
    ///
    /// Note: This method uses default command timeouts. For movement detection
    /// within a bounded time budget, use `is_moving_with_deadline` instead.
    pub fn is_moving(&mut self) -> Result<bool, Error> {
        // Use a generous 30 second deadline for unbounded is_moving
        let deadline = Deadline::from_timeout(Duration::from_secs(30));
        self.is_moving_with_deadline(deadline)
    }

    /// Check if the camera is currently moving, respecting an external deadline.
    ///
    /// This variant ensures that position inquiries don't exceed the given
    /// deadline, which is essential for proper timeout behavior in
    /// `wait_for_movement` and related methods.
    ///
    /// This is equivalent to
    /// `is_moving_axes_with_deadline(Axes::ALL, deadline, &MovementTolerance::default())`.
    ///
    /// # Arguments
    ///
    /// * `deadline` - The deadline by which all inquiries must complete
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Camera is moving (positions changed between samples)
    /// * `Ok(false)` - Camera is idle (positions stable)
    /// * `Err(Error::Timeout)` - Deadline exceeded before completing check
    /// * `Err(...)` - Communication or parse error
    pub fn is_moving_with_deadline(&mut self, deadline: Deadline) -> Result<bool, Error> {
        self.is_moving_axes_with_deadline(Axes::ALL, deadline, &MovementTolerance::default())
    }

    /// Check if specific camera axes are currently moving, respecting an external deadline.
    ///
    /// This is more efficient than `is_moving_with_deadline` when you only care about
    /// specific axes (e.g., only pan/tilt after a pan_tilt_absolute command).
    ///
    /// # Arguments
    ///
    /// * `axes` - Which axes to check for movement
    /// * `deadline` - The deadline by which all inquiries must complete
    /// * `tolerance` - Movement tolerance values
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - At least one monitored axis is moving
    /// * `Ok(false)` - All monitored axes are idle
    /// * `Err(Error::Timeout)` - Deadline exceeded before completing check
    /// * `Err(...)` - Communication or parse error
    pub fn is_moving_axes_with_deadline(
        &self,
        axes: Axes,
        deadline: Deadline,
        tolerance: &MovementTolerance,
    ) -> Result<bool, Error> {
        if axes.is_empty() {
            return Ok(false);
        }

        if deadline.is_expired() {
            return Err(Error::Timeout);
        }

        let mut pt_moving = false;
        let mut zoom_moving = false;
        let mut focus_moving = false;

        // First sample - only query the axes we care about
        let pos1_pt = if axes.has_pan_tilt() {
            let response = self
                .send_command_with_deadline(&PanTiltPositionInquiry, deadline)
                .block()?;
            match response {
                crate::command::Response::Inquiry(
                    crate::command::InquiryData::PanTiltPosition { pan, tilt },
                ) => Some((pan, tilt)),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            }
        } else {
            None
        };

        if deadline.is_expired() {
            return Err(Error::Timeout);
        }

        let pos1_zoom = if axes.has_zoom() {
            let response = self
                .send_command_with_deadline(&ZoomPositionInquiry, deadline)
                .block()?;
            match response {
                crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                    position,
                }) => Some(position),
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            }
        } else {
            None
        };

        if deadline.is_expired() {
            return Err(Error::Timeout);
        }

        let pos1_focus = if axes.has_focus() {
            let response = self
                .send_command_with_deadline(&FocusPositionInquiry, deadline)
                .block()?;
            match response {
                crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                    position,
                }) => Some(position),
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            }
        } else {
            None
        };

        // Second sample - the I/O latency from the first set of inquiries
        // provides sufficient delay for motion detection
        if let Some((pos1_pan, pos1_tilt)) = pos1_pt {
            let response = self
                .send_command_with_deadline(&PanTiltPositionInquiry, deadline)
                .block()?;
            let (pos2_pan, pos2_tilt) = match response {
                crate::command::Response::Inquiry(
                    crate::command::InquiryData::PanTiltPosition { pan, tilt },
                ) => (pan, tilt),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            };
            pt_moving = !positions_equal_within_tolerance(
                PanTiltPosition {
                    pan: pos1_pan,
                    tilt: pos1_tilt,
                },
                PanTiltPosition {
                    pan: pos2_pan,
                    tilt: pos2_tilt,
                },
                tolerance.pan_tilt,
            );
        }

        if deadline.is_expired() {
            return Err(Error::Timeout);
        }

        if let Some(pos1_z) = pos1_zoom {
            let response = self
                .send_command_with_deadline(&ZoomPositionInquiry, deadline)
                .block()?;
            let pos2_z = match response {
                crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            };
            zoom_moving = !zoom_equal_within_tolerance(pos1_z, pos2_z, tolerance.zoom);
        }

        if deadline.is_expired() {
            return Err(Error::Timeout);
        }

        if let Some(pos1_f) = pos1_focus {
            let response = self
                .send_command_with_deadline(&FocusPositionInquiry, deadline)
                .block()?;
            let pos2_f = match response {
                crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            };
            focus_moving = (pos1_f as i32 - pos2_f as i32).abs() > tolerance.focus as i32;
        }

        Ok(pt_moving || zoom_moving || focus_moving)
    }

    /// Poll the given axes until they stop moving or the deadline elapses.
    ///
    /// Crate-internal helper backing
    /// [`BlockingInFlight::await_settled`](crate::camera::BlockingInFlight::await_settled)
    /// on profiles that do not emit an operation-complete message. Runs entirely
    /// on the caller's thread using position inquiries.
    ///
    /// # Errors
    /// Returns [`Error::Timeout`] if motion does not settle before `deadline`, or
    /// a communication / parse error otherwise.
    pub(crate) fn await_axes_settled(&self, axes: Axes, deadline: Deadline) -> Result<(), Error> {
        if axes.is_empty() {
            return Ok(());
        }
        let tolerance = MovementTolerance::default();
        loop {
            if deadline.is_expired() {
                return Err(Error::Timeout);
            }
            if !self.is_moving_axes_with_deadline(axes, deadline, &tolerance)? {
                return Ok(());
            }
            // Space out samples; the inquiry round-trips already add latency, but
            // a short sleep avoids hammering the camera on fast links.
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Wait for movement completion using an [`AwaitConfig`].
    ///
    /// This is the most flexible movement waiting API, allowing fine-grained
    /// control over which axes to monitor, timeout, debug logging, and tolerances.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the wait operation
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All monitored axes have stopped moving
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    /// * `Err(...)` - Communication or camera error
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Wait for preset recall with appropriate timeout
    /// camera.await_with_config(&AwaitConfig::for_preset_recall())?;
    ///
    /// // Custom configuration
    /// let config = AwaitConfig::new(Duration::from_secs(20))
    ///     .with_axes(Axes::PAN_TILT | Axes::ZOOM)
    ///     .with_debug();
    /// camera.await_with_config(&config)?;
    /// ```
    pub fn await_with_config(&mut self, config: &AwaitConfig) -> Result<(), Error> {
        if config.axes.is_empty() {
            return Ok(());
        }

        let deadline = Deadline::from_timeout(config.timeout);

        if config.debug {
            tracing::debug!(
                "Waiting for movement completion: axes={}, timeout={:?}",
                config.axes,
                config.timeout
            );
        }

        loop {
            if deadline.is_expired() {
                if config.debug {
                    tracing::debug!("Movement detection timed out");
                }
                return Err(Error::Timeout);
            }

            match self.is_moving_axes_with_deadline(config.axes, deadline, &config.tolerance) {
                Ok(false) => {
                    if config.debug {
                        tracing::debug!("Movement completed (all monitored axes idle)");
                    }
                    return Ok(());
                }
                Ok(true) => {
                    // Still moving, continue polling
                    if config.debug {
                        tracing::trace!("Still moving, continuing to poll...");
                    }
                }
                Err(Error::Timeout) => {
                    if config.debug {
                        tracing::debug!("Movement detection timed out during polling");
                    }
                    return Err(Error::Timeout);
                }
                Err(e) => {
                    return Err(e);
                }
            }

            // Sleep between polls, capped by remaining time
            let sleep_time = config.poll_interval.min(deadline.remaining());
            if sleep_time > Duration::ZERO {
                std::thread::sleep(sleep_time);
            }
        }
    }

    /// Wait for specific axes to become idle.
    ///
    /// This is a convenience method that creates an [`AwaitConfig`] with the
    /// specified axes and timeout.
    ///
    /// # Arguments
    ///
    /// * `axes` - Which axes to monitor
    /// * `timeout` - Maximum time to wait
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // After pan/tilt command, wait only for pan/tilt
    /// camera.pan_tilt_absolute(Degrees(45.0), Degrees(10.0), SpeedLevel::Fast)?;
    /// camera.await_axes_idle(Axes::PAN_TILT, Duration::from_secs(20))?;
    ///
    /// // After preset recall, wait for all axes
    /// camera.preset_recall(PresetNumber::new(1)?)?;
    /// camera.await_axes_idle(Axes::ALL, Duration::from_secs(60))?;
    /// ```
    pub fn await_axes_idle(&mut self, axes: Axes, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::new(timeout).with_axes(axes))
    }
}

#[cfg(feature = "mode-async")]
impl<P, T, E> Camera<crate::mode::Async, P, T, E>
where
    P: Profile + ProfileMetadata + Default,
    T: AsyncTransport + Send + Sync + 'static,
    E: Executor,
{
    /// Wait for pan/tilt movement to complete (async version).
    ///
    /// Convenience method that waits for pan and tilt motors to stop moving.
    /// This delegates to [`Self::await_with_config`] with [`AwaitConfig::for_pan_tilt()`]
    /// settings and the specified timeout.
    pub async fn await_pan_tilt_idle(&self, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::for_pan_tilt().with_timeout(timeout))
            .await
    }

    /// Wait for zoom movement to complete (async version).
    ///
    /// Convenience method that waits for zoom motor to stop moving.
    /// This delegates to [`Self::await_with_config`] with [`AwaitConfig::for_zoom()`]
    /// settings and the specified timeout.
    pub async fn await_zoom_idle(&self, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::for_zoom().with_timeout(timeout))
            .await
    }

    /// Wait for focus movement to complete (async version).
    ///
    /// Convenience method that waits for focus motor to stop moving.
    /// This delegates to [`Self::await_with_config`] with [`AwaitConfig::for_focus()`]
    /// settings and the specified timeout.
    pub async fn await_focus_idle(&self, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::for_focus().with_timeout(timeout))
            .await
    }

    /// Wait for all movements to complete (async version).
    ///
    /// Convenience method that waits for all motors (pan/tilt, zoom, focus) to stop.
    /// This delegates to [`Self::await_with_config`] with all axes and the specified timeout.
    ///
    /// For more control over which axes to monitor, use [`Self::await_with_config`] or
    /// [`Self::await_axes_idle`].
    pub async fn await_idle(&self, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::new(timeout)).await
    }

    /// Check if the camera is currently moving (async version).
    ///
    /// This is equivalent to `is_moving_axes_async(Axes::ALL, &MovementTolerance::default())`.
    pub async fn is_moving_async(&self) -> Result<bool, Error> {
        self.is_moving_axes_async(Axes::ALL, &MovementTolerance::default())
            .await
    }

    /// Check if specific camera axes are currently moving (async version).
    ///
    /// This is more efficient than `is_moving_async` when you only care about
    /// specific axes (e.g., only pan/tilt after a pan_tilt_absolute command).
    ///
    /// Movement is detected by taking two position samples and comparing them.
    /// The time between samples is determined by the I/O latency of the position
    /// inquiries themselves, which provides sufficient delay for detecting motion.
    pub async fn is_moving_axes_async(
        &self,
        axes: Axes,
        tolerance: &MovementTolerance,
    ) -> Result<bool, Error> {
        if axes.is_empty() {
            return Ok(false);
        }

        let mut pt_moving = false;
        let mut zoom_moving = false;
        let mut focus_moving = false;

        // First sample - only query the axes we care about
        let pos1_pt = if axes.has_pan_tilt() {
            let response = self.send_command(&PanTiltPositionInquiry).await?;
            match response {
                crate::command::Response::Inquiry(
                    crate::command::InquiryData::PanTiltPosition { pan, tilt },
                ) => Some((pan, tilt)),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            }
        } else {
            None
        };

        let pos1_zoom = if axes.has_zoom() {
            let response = self.send_command(&ZoomPositionInquiry).await?;
            match response {
                crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                    position,
                }) => Some(position),
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            }
        } else {
            None
        };

        let pos1_focus = if axes.has_focus() {
            let response = self.send_command(&FocusPositionInquiry).await?;
            match response {
                crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                    position,
                }) => Some(position),
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            }
        } else {
            None
        };

        // Second sample - the I/O latency from the first set of inquiries
        // provides sufficient delay for motion detection
        if let Some((pos1_pan, pos1_tilt)) = pos1_pt {
            let response = self.send_command(&PanTiltPositionInquiry).await?;
            let (pos2_pan, pos2_tilt) = match response {
                crate::command::Response::Inquiry(
                    crate::command::InquiryData::PanTiltPosition { pan, tilt },
                ) => (pan, tilt),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            };
            pt_moving = !positions_equal_within_tolerance(
                PanTiltPosition {
                    pan: pos1_pan,
                    tilt: pos1_tilt,
                },
                PanTiltPosition {
                    pan: pos2_pan,
                    tilt: pos2_tilt,
                },
                tolerance.pan_tilt,
            );
        }

        if let Some(pos1_z) = pos1_zoom {
            let response = self.send_command(&ZoomPositionInquiry).await?;
            let pos2_z = match response {
                crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            };
            zoom_moving = !zoom_equal_within_tolerance(pos1_z, pos2_z, tolerance.zoom);
        }

        if let Some(pos1_f) = pos1_focus {
            let response = self.send_command(&FocusPositionInquiry).await?;
            let pos2_f = match response {
                crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            };
            focus_moving = (pos1_f as i32 - pos2_f as i32).abs() > tolerance.focus as i32;
        }

        Ok(pt_moving || zoom_moving || focus_moving)
    }

    /// Wait for movement completion using an [`AwaitConfig`] (async version).
    ///
    /// This is the canonical movement waiting API, providing fine-grained control
    /// over which axes to monitor, timeout, debug logging, and tolerances.
    ///
    /// When the camera supports VISCA completion messages, this method uses
    /// event-driven detection for faster response. Otherwise, it falls back to
    /// efficient state-query polling with configurable `poll_interval`.
    pub async fn await_with_config(&self, config: &AwaitConfig) -> Result<(), Error> {
        if config.axes.is_empty() {
            return Ok(());
        }

        if config.debug {
            tracing::debug!(
                "Waiting for movement completion: axes={}, timeout={:?}, poll_interval={:?}",
                config.axes,
                config.timeout,
                config.poll_interval
            );
        }

        // Try event-driven detection if supported
        if P::SUPPORTS_OPERATION_COMPLETE {
            if config.debug {
                tracing::debug!(
                    "Using event-driven movement detection (camera supports completion messages)"
                );
            }

            match self.await_with_config_event_driven(config).await {
                Ok(()) => {
                    if config.debug {
                        tracing::debug!("Movement completed via completion event");
                    }
                    return Ok(());
                }
                Err(Error::NotSupported) => {
                    if config.debug {
                        tracing::debug!(
                            "Event-driven detection unavailable, falling back to polling"
                        );
                    }
                }
                Err(e) => return Err(e),
            }
        }

        // Fall back to state-query polling
        if config.debug {
            tracing::debug!(
                "Using state-query polling with {}ms interval",
                config.poll_interval.as_millis()
            );
        }

        self.await_with_config_polling(config).await
    }

    /// Event-driven movement detection using runtime completion events.
    async fn await_with_config_event_driven(&self, config: &AwaitConfig) -> Result<(), Error> {
        use crate::timeout::CommandCategory;

        let completion_rx = match self.runtime().subscribe_completions().await {
            Ok(rx) => rx,
            Err(_) => return Err(Error::NotSupported),
        };

        let start = self.runtime().executor().now();

        while self
            .runtime()
            .executor()
            .now()
            .saturating_duration_since(start)
            < config.timeout
        {
            match completion_rx.try_recv() {
                Ok(event) => {
                    if event.camera_id != self.camera_id() {
                        continue;
                    }

                    // Check if this is a relevant completion event
                    let is_relevant = matches!(
                        event.category,
                        CommandCategory::Movement | CommandCategory::Preset
                    );

                    if is_relevant {
                        if config.debug {
                            tracing::debug!("Received {:?} completion event", event.category);
                        }

                        // Small settling delay after completion message
                        self.sleep(Duration::from_millis(50)).await;

                        // Verify the monitored axes are actually idle
                        let is_moving = self
                            .is_moving_axes_async(config.axes, &config.tolerance)
                            .await?;
                        if !is_moving {
                            if config.debug {
                                tracing::debug!("Camera confirmed idle after completion event");
                            }
                            return Ok(());
                        } else if config.debug {
                            tracing::debug!(
                                "Camera still moving after completion, waiting for more events"
                            );
                        }
                    }
                }
                Err(flume::TryRecvError::Empty) => {
                    // No events available, sleep briefly before checking again
                    self.sleep(Duration::from_millis(50)).await;
                }
                Err(flume::TryRecvError::Disconnected) => return Err(Error::NotSupported),
            }
        }

        Err(Error::Timeout)
    }

    /// Polling-based movement detection using configurable poll interval.
    async fn await_with_config_polling(&self, config: &AwaitConfig) -> Result<(), Error> {
        let start = self.runtime().executor().now();

        loop {
            let elapsed = self
                .runtime()
                .executor()
                .now()
                .saturating_duration_since(start);
            if elapsed >= config.timeout {
                if config.debug {
                    tracing::debug!("Movement detection timed out");
                }
                return Err(Error::Timeout);
            }

            match self
                .is_moving_axes_async(config.axes, &config.tolerance)
                .await
            {
                Ok(false) => {
                    if config.debug {
                        tracing::debug!("Movement completed (all monitored axes idle)");
                    }
                    return Ok(());
                }
                Ok(true) => {
                    if config.debug {
                        tracing::trace!("Still moving, continuing to poll...");
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }

            // Sleep between polls, capped by remaining timeout
            let remaining = config.timeout.saturating_sub(elapsed);
            let sleep_time = config.poll_interval.min(remaining);
            if sleep_time > Duration::ZERO {
                self.sleep(sleep_time).await;
            }
        }
    }

    /// Wait for specific axes to become idle (async version).
    ///
    /// This is a convenience method that creates an [`AwaitConfig`] with the
    /// specified axes and timeout.
    pub async fn await_axes_idle(&self, axes: Axes, timeout: Duration) -> Result<(), Error> {
        self.await_with_config(&AwaitConfig::new(timeout).with_axes(axes))
            .await
    }
}
