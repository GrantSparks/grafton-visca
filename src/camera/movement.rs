//! Movement detection utilities and configuration.
//!
//! This module provides a comprehensive movement detection API that uses
//! VISCA completion messages when available and falls back to efficient state
//! querying when needed. It includes utilities for position comparison,
//! movement configuration, and both event-driven and state-based detection.

use std::time::{Duration, Instant};

use super::Camera;
use crate::{
    capabilities::{Profile, ProfileMetadata},
    command::inquiry::{FocusPositionInquiry, PanTiltPositionInquiry, ZoomPositionInquiry},
    error::Error,
};
#[cfg(feature = "mode-async")]
use crate::{executor::Executor, transport::AsyncTransport};
#[cfg(not(feature = "mode-async"))]
use crate::{mode::BlockingFutureExt, transport::BlockingTransport};

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

// Blocking mode implementation is only available without async feature
#[cfg(not(feature = "mode-async"))]
impl<P, T> Camera<crate::mode::Blocking, P, T, ()>
where
    P: Profile + ProfileMetadata + Default,
    T: BlockingTransport + crate::transport::HasTransportConfig + 'static,
{
    /// Wait for a command completion message or idle state using the default timeout.
    ///
    /// Uses `TimeoutConfig::movement_timeout` as the default limit and falls back
    /// to state-query detection in blocking mode.
    pub fn wait_for_completion(&mut self) -> Result<(), Error> {
        let timeout = self.timeout_config().movement_timeout;
        self.wait_for_completion_with_timeout(timeout)
    }

    /// Wait for a command completion message or idle state with a custom timeout.
    ///
    /// This mirrors the async API and provides a convenient entry point for
    /// movement waits in blocking mode.
    pub fn wait_for_completion_with_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };
        self.wait_for_movement(&config)
    }

    /// Wait for any movement operation to complete.
    ///
    /// This method uses VISCA completion messages (0x51) when supported by the camera
    /// and async features are enabled, providing instant response when movement finishes.
    /// Otherwise, it uses efficient state querying.
    ///
    /// # Arguments
    /// * `config` - Movement detection configuration
    ///
    /// # Returns
    /// * `Ok(())` - Movement completed successfully
    /// * `Err(Error::Timeout)` - Movement did not complete within timeout
    /// * `Err(Error::*)` - Other communication or camera errors
    pub fn wait_for_movement(&mut self, config: &MovementConfig) -> Result<(), Error> {
        // In blocking mode, we can't use event-driven detection with async channels
        // Users should use the async wait_for_movement method for event-driven detection

        // Use state querying (works in both blocking and async modes)
        if config.debug {
            tracing::debug!("Using state-query movement detection");
        }

        self.wait_using_state_query(config)
    }

    /// Wait for movement using state querying (fallback method).
    fn wait_using_state_query(&mut self, config: &MovementConfig) -> Result<(), Error> {
        let start = Instant::now();

        // Keep checking if camera is moving
        loop {
            if start.elapsed() > config.timeout {
                if config.debug {
                    tracing::debug!("Movement detection timed out");
                }
                return Err(Error::Timeout);
            }

            if !self.is_moving()? {
                if config.debug {
                    tracing::debug!("Movement completed (camera is idle)");
                }
                return Ok(());
            }

            // Sleep briefly to avoid busy-waiting while polling state
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Wait for pan/tilt movement to complete.
    ///
    /// Convenience method that waits for pan and tilt motors to stop moving.
    pub fn await_pan_tilt_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };

        // Keep checking until pan/tilt stops moving
        let start = Instant::now();
        if config.debug {
            tracing::debug!(
                "Waiting for pan/tilt movement to complete (timeout: {:?})",
                config.timeout
            );
        }

        // Initial delay to let movement start
        std::thread::sleep(Duration::from_millis(100));

        // Progressive polling intervals: start fast, then slow down
        let mut poll_interval = Duration::from_millis(100);
        let max_interval = Duration::from_millis(500);

        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only pan/tilt movement
            let pos1_response = self.send_command(&PanTiltPositionInquiry).block()?;
            let (pos1_pan, pos1_tilt) = match pos1_response {
                crate::command::Response::Inquiry(
                    crate::command::InquiryData::PanTiltPosition { pan, tilt },
                ) => (pan, tilt),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            };

            // Wait with progressive backoff
            std::thread::sleep(poll_interval);

            let pos2_response = self.send_command(&PanTiltPositionInquiry).block()?;
            let (pos2_pan, pos2_tilt) = match pos2_response {
                crate::command::Response::Inquiry(
                    crate::command::InquiryData::PanTiltPosition { pan, tilt },
                ) => (pan, tilt),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            };

            // Check if position is stable (not moving)
            if (pos1_pan - pos2_pan).abs() <= 2 && (pos1_tilt - pos2_tilt).abs() <= 2 {
                return Ok(());
            }

            // Increase polling interval up to maximum
            if poll_interval < max_interval {
                poll_interval = (poll_interval * 3 / 2).min(max_interval);
            }
        }
    }

    /// Wait for zoom movement to complete.
    ///
    /// Convenience method that waits for zoom motor to stop moving.
    pub fn await_zoom_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };

        // Keep checking until zoom stops moving
        let start = Instant::now();
        if config.debug {
            tracing::debug!(
                "Waiting for zoom movement to complete (timeout: {:?})",
                config.timeout
            );
        }
        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only zoom movement
            let pos1_response = self.send_command(&ZoomPositionInquiry).block()?;
            let pos1_zoom = match pos1_response {
                crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            };

            std::thread::sleep(Duration::from_millis(50));

            let pos2_response = self.send_command(&ZoomPositionInquiry).block()?;
            let pos2_zoom = match pos2_response {
                crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            };

            // Check if position is stable (not moving)
            if (pos1_zoom as i32 - pos2_zoom as i32).abs() <= 10 {
                return Ok(());
            }
        }
    }

    /// Wait for focus movement to complete.
    ///
    /// Convenience method that waits for focus motor to stop moving.
    pub fn await_focus_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };

        // Keep checking until focus stops moving
        let start = Instant::now();
        if config.debug {
            tracing::debug!(
                "Waiting for focus movement to complete (timeout: {:?})",
                config.timeout
            );
        }
        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only focus movement
            let pos1_response = self.send_command(&FocusPositionInquiry).block()?;
            let pos1_focus = match pos1_response {
                crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            };

            std::thread::sleep(Duration::from_millis(50));

            let pos2_response = self.send_command(&FocusPositionInquiry).block()?;
            let pos2_focus = match pos2_response {
                crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            };

            // Check if position is stable (not moving)
            if (pos1_focus as i32 - pos2_focus as i32).abs() <= 5 {
                return Ok(());
            }
        }
    }

    /// Wait for all movements to complete.
    ///
    /// Convenience method that waits for all motors (pan/tilt, zoom, focus) to stop.
    pub fn await_idle(&mut self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };
        self.wait_for_movement(&config)
    }

    /// Check if the camera is currently moving.
    ///
    /// This checks pan/tilt, zoom, and focus positions to detect movement.
    pub fn is_moving(&mut self) -> Result<bool, Error> {
        // Get first reading using inquiry commands
        let pos1_pt_response = self.send_command(&PanTiltPositionInquiry).block()?;
        let (pos1_pan, pos1_tilt) = match pos1_pt_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::PanTiltPosition {
                pan,
                tilt,
            }) => (pan, tilt),
            _ => {
                return Err(Error::ParseError(
                    "Expected PanTiltPosition response".into(),
                ))
            }
        };

        let pos1_zoom_response = self.send_command(&ZoomPositionInquiry).block()?;
        let pos1_zoom = match pos1_zoom_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                position,
            }) => position,
            _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
        };

        let pos1_focus_response = self.send_command(&FocusPositionInquiry).block()?;
        let pos1_focus = match pos1_focus_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                position,
            }) => position,
            _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
        };

        // Sleep briefly to allow state to change between samples
        std::thread::sleep(Duration::from_millis(5));

        // Get second reading
        let pos2_pt_response = self.send_command(&PanTiltPositionInquiry).block()?;
        let (pos2_pan, pos2_tilt) = match pos2_pt_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::PanTiltPosition {
                pan,
                tilt,
            }) => (pan, tilt),
            _ => {
                return Err(Error::ParseError(
                    "Expected PanTiltPosition response".into(),
                ))
            }
        };

        let pos2_zoom_response = self.send_command(&ZoomPositionInquiry).block()?;
        let pos2_zoom = match pos2_zoom_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                position,
            }) => position,
            _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
        };

        let pos2_focus_response = self.send_command(&FocusPositionInquiry).block()?;
        let pos2_focus = match pos2_focus_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                position,
            }) => position,
            _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
        };

        // Check for movement with reasonable tolerances
        let pt_moving = !positions_equal_within_tolerance(
            PanTiltPosition {
                pan: pos1_pan,
                tilt: pos1_tilt,
            },
            PanTiltPosition {
                pan: pos2_pan,
                tilt: pos2_tilt,
            },
            2, // 2 units tolerance for pan/tilt
        );

        let zoom_moving = (pos1_zoom as i32 - pos2_zoom as i32).abs() > 10; // 10 units tolerance for zoom
        let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5; // 5 units tolerance for focus

        Ok(pt_moving || zoom_moving || focus_moving)
    }
}

#[cfg(feature = "mode-async")]
impl<P, T, E> Camera<crate::mode::Async, P, T, E>
where
    P: Profile + ProfileMetadata + Default,
    T: AsyncTransport + Send + Sync + 'static,
    E: Executor,
{
    /// Wait for any movement operation to complete (async version).
    ///
    /// This method uses VISCA completion messages (0x51) when supported by the camera,
    /// providing instant response when movement finishes. For cameras without this
    /// support, it falls back to efficient state querying.
    pub async fn wait_for_movement_async(&self, config: &MovementConfig) -> Result<(), Error> {
        // Check if camera supports operation complete messages
        if P::SUPPORTS_OPERATION_COMPLETE {
            if config.debug {
                tracing::debug!(
                    "Using event-driven movement detection (camera supports completion messages)"
                );
            }

            // Try to wait for completion message
            // Respect MovementConfig timeout when waiting for completion
            match self.wait_for_movement_internal(config).await {
                Ok(()) => {
                    if config.debug {
                        tracing::debug!("Movement completed (received operation complete message)");
                    }
                    return Ok(());
                }
                Err(Error::NotSupported) => {
                    // Transport doesn't support waiting for completion
                    if config.debug {
                        tracing::debug!("Transport doesn't support event-driven detection, falling back to state query");
                    }
                }
                Err(Error::Timeout) => {
                    // No completion message within timeout
                    if config.debug {
                        tracing::debug!(
                            "No completion message received, falling back to state query"
                        );
                    }
                }
                Err(e) => {
                    // Other error, propagate it
                    return Err(e);
                }
            }
        }

        // Fallback: Use state querying
        if config.debug {
            tracing::debug!("Using state-query movement detection");
        }

        self.wait_using_state_query_async(config).await
    }

    /// Wait for movement using state querying (async fallback method).
    async fn wait_using_state_query_async(&self, config: &MovementConfig) -> Result<(), Error> {
        let start = Instant::now();

        // Keep checking if camera is moving
        loop {
            if start.elapsed() > config.timeout {
                if config.debug {
                    tracing::debug!("Movement detection timed out");
                }
                return Err(Error::Timeout);
            }

            if !self.is_moving_async().await? {
                if config.debug {
                    tracing::debug!("Movement completed (camera is idle)");
                }
                return Ok(());
            }

            // Yield to scheduler using a short sleep to avoid busy looping
            // Keep cadence in line with blocking path (~5ms)
            self.sleep(Duration::from_millis(5)).await;
        }
    }

    /// Wait for pan/tilt movement to complete (async version).
    ///
    /// Convenience method that waits for pan and tilt motors to stop moving.
    pub async fn await_pan_tilt_idle(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };

        // Keep checking until pan/tilt stops moving
        let start = Instant::now();
        if config.debug {
            tracing::debug!(
                "Waiting for pan/tilt movement to complete (timeout: {:?})",
                config.timeout
            );
        }

        // Initial delay to let movement start
        self.sleep(Duration::from_millis(100)).await;

        // Progressive polling intervals: start fast, then slow down
        let mut poll_interval = Duration::from_millis(100);
        let max_interval = Duration::from_millis(500);

        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only pan/tilt movement
            let pos1_response = self.send_command(&PanTiltPositionInquiry).await?;
            let (pos1_pan, pos1_tilt) = match pos1_response {
                crate::command::Response::Inquiry(
                    crate::command::InquiryData::PanTiltPosition { pan, tilt },
                ) => (pan, tilt),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            };

            // Wait with progressive backoff
            self.sleep(poll_interval).await;

            let pos2_response = self.send_command(&PanTiltPositionInquiry).await?;
            let (pos2_pan, pos2_tilt) = match pos2_response {
                crate::command::Response::Inquiry(
                    crate::command::InquiryData::PanTiltPosition { pan, tilt },
                ) => (pan, tilt),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            };

            // Check if position is stable (not moving)
            if (pos1_pan - pos2_pan).abs() <= 2 && (pos1_tilt - pos2_tilt).abs() <= 2 {
                return Ok(());
            }

            // Increase polling interval up to maximum
            if poll_interval < max_interval {
                poll_interval = (poll_interval * 3 / 2).min(max_interval);
            }
        }
    }

    /// Wait for zoom movement to complete (async version).
    ///
    /// Convenience method that waits for zoom motor to stop moving.
    pub async fn await_zoom_idle(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };

        // Keep checking until zoom stops moving
        let start = Instant::now();
        if config.debug {
            tracing::debug!(
                "Waiting for zoom movement to complete (timeout: {:?})",
                config.timeout
            );
        }
        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only zoom movement
            let pos1_response = self.send_command(&ZoomPositionInquiry).await?;
            let pos1_zoom = match pos1_response {
                crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            };

            self.sleep(Duration::from_millis(50)).await;

            let pos2_response = self.send_command(&ZoomPositionInquiry).await?;
            let pos2_zoom = match pos2_response {
                crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            };

            // Check if position is stable (not moving)
            if (pos1_zoom as i32 - pos2_zoom as i32).abs() <= 10 {
                return Ok(());
            }
        }
    }

    /// Wait for focus movement to complete (async version).
    ///
    /// Convenience method that waits for focus motor to stop moving.
    pub async fn await_focus_idle(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };

        // Keep checking until focus stops moving
        let start = Instant::now();
        if config.debug {
            tracing::debug!(
                "Waiting for focus movement to complete (timeout: {:?})",
                config.timeout
            );
        }
        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only focus movement
            let pos1_response = self.send_command(&FocusPositionInquiry).await?;
            let pos1_focus = match pos1_response {
                crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            };

            self.sleep(Duration::from_millis(50)).await;

            let pos2_response = self.send_command(&FocusPositionInquiry).await?;
            let pos2_focus = match pos2_response {
                crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                    position,
                }) => position,
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            };

            // Check if position is stable (not moving)
            if (pos1_focus as i32 - pos2_focus as i32).abs() <= 5 {
                return Ok(());
            }
        }
    }

    /// Wait for a command completion message or idle state with a custom timeout (async version).
    ///
    /// This mirrors the blocking API and provides a convenient entry point for
    /// movement waits in async mode.
    pub async fn wait_for_completion_with_timeout(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };
        self.wait_for_movement_internal(&config).await
    }

    /// Internal helper for movement detection that doesn't call public APIs.
    async fn wait_for_movement_internal(&self, config: &MovementConfig) -> Result<(), Error> {
        // Check if camera supports operation complete messages
        if P::SUPPORTS_OPERATION_COMPLETE {
            if config.debug {
                tracing::debug!(
                    "Using event-driven movement detection (camera supports completion messages)"
                );
            }

            // Try to use event-driven approach
            match self.wait_for_movement_event_driven(config).await {
                Ok(()) => return Ok(()),
                Err(Error::NotSupported) => {
                    // Fall through to polling approach
                    if config.debug {
                        tracing::debug!(
                            "Event-driven approach unavailable, falling back to polling"
                        );
                    }
                }
                Err(e) => return Err(e),
            }
        }

        // Fall back to state querying for all other cameras
        if config.debug {
            tracing::debug!("Using state-query movement detection (fallback mode)");
        }

        let start = Instant::now();

        // Loop until timeout checking if camera has stopped moving
        while start.elapsed() < config.timeout {
            // Check if we're still moving
            let moving = self.is_moving_async().await?;
            if !moving {
                return Ok(());
            }

            // Wait a bit before checking again
            self.sleep(Duration::from_millis(100)).await;
        }

        Err(Error::Timeout)
    }

    /// Event-driven movement detection using runtime completion events.
    async fn wait_for_movement_event_driven(&self, config: &MovementConfig) -> Result<(), Error> {
        use crate::timeout::CommandCategory;

        // Subscribe to completion events from the runtime
        let completion_rx = match self.runtime().subscribe_completions().await {
            Ok(rx) => rx,
            Err(_) => return Err(Error::NotSupported),
        };

        let start = Instant::now();
        let mut seen_movement_completion = false;
        let mut seen_preset_completion = false;

        // Wait for completion events with timeout
        while start.elapsed() < config.timeout {
            // Try to receive a completion event (non-blocking first)
            match completion_rx.try_recv() {
                Ok(event) => {
                    // Check if this is our camera
                    if event.camera_id != self.camera_id() {
                        continue;
                    }

                    // Track completions by category
                    match event.category {
                        CommandCategory::Movement => {
                            seen_movement_completion = true;
                            if config.debug {
                                tracing::debug!("Received movement completion event");
                            }
                        }
                        CommandCategory::Preset => {
                            seen_preset_completion = true;
                            if config.debug {
                                tracing::debug!("Received preset completion event");
                            }
                        }
                        _ => continue,
                    }

                    // After receiving at least one relevant completion, confirm idle state
                    if seen_movement_completion || seen_preset_completion {
                        // Give a small delay for any final settling
                        self.sleep(Duration::from_millis(50)).await;

                        // Confirm the camera is actually idle
                        let is_moving = self.is_moving_async().await?;
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
                    // No events available right now, wait a bit
                    self.sleep(Duration::from_millis(50)).await;
                }
                Err(flume::TryRecvError::Disconnected) => {
                    // Channel closed
                    return Err(Error::NotSupported);
                }
            }
        }

        // If we saw at least one completion but never confirmed idle, that's still a timeout
        Err(Error::Timeout)
    }

    /// Wait for all movements to complete (async version).
    ///
    /// Convenience method that waits for all motors (pan/tilt, zoom, focus) to stop.
    pub async fn await_idle(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementConfig {
            timeout,
            debug: false,
        };
        self.wait_for_movement_internal(&config).await
    }

    /// Check if the camera is currently moving (async version).
    pub async fn is_moving_async(&self) -> Result<bool, Error> {
        // Get first reading using inquiry commands
        let pos1_pt_response = self.send_command(&PanTiltPositionInquiry).await?;
        let (pos1_pan, pos1_tilt) = match pos1_pt_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::PanTiltPosition {
                pan,
                tilt,
            }) => (pan, tilt),
            _ => {
                return Err(Error::ParseError(
                    "Expected PanTiltPosition response".into(),
                ))
            }
        };

        let pos1_zoom_response = self.send_command(&ZoomPositionInquiry).await?;
        let pos1_zoom = match pos1_zoom_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                position,
            }) => position,
            _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
        };

        let pos1_focus_response = self.send_command(&FocusPositionInquiry).await?;
        let pos1_focus = match pos1_focus_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                position,
            }) => position,
            _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
        };

        // Yield to scheduler
        self.sleep(Duration::from_millis(5)).await;

        // Get second reading
        let pos2_pt_response = self.send_command(&PanTiltPositionInquiry).await?;
        let (pos2_pan, pos2_tilt) = match pos2_pt_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::PanTiltPosition {
                pan,
                tilt,
            }) => (pan, tilt),
            _ => {
                return Err(Error::ParseError(
                    "Expected PanTiltPosition response".into(),
                ))
            }
        };

        let pos2_zoom_response = self.send_command(&ZoomPositionInquiry).await?;
        let pos2_zoom = match pos2_zoom_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::ZoomPosition {
                position,
            }) => position,
            _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
        };

        let pos2_focus_response = self.send_command(&FocusPositionInquiry).await?;
        let pos2_focus = match pos2_focus_response {
            crate::command::Response::Inquiry(crate::command::InquiryData::FocusPosition {
                position,
            }) => position,
            _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
        };

        // Check for movement with reasonable tolerances
        let pt_moving = !positions_equal_within_tolerance(
            PanTiltPosition {
                pan: pos1_pan,
                tilt: pos1_tilt,
            },
            PanTiltPosition {
                pan: pos2_pan,
                tilt: pos2_tilt,
            },
            2, // 2 units tolerance for pan/tilt
        );

        let zoom_moving = (pos1_zoom as i32 - pos2_zoom as i32).abs() > 10; // 10 units tolerance for zoom
        let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5; // 5 units tolerance for focus

        Ok(pt_moving || zoom_moving || focus_moving)
    }
}
