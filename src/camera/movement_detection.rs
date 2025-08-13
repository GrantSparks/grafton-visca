//! Event-driven movement detection for PTZ cameras.
//!
//! This module provides efficient movement detection using VISCA completion
//! messages when available, with automatic fallback to state querying.

use std::time::Instant;

use crate::{
    capabilities::{Profile, ProfileMetadata},
    error::Error,
};

use crate::transport::BlockingTransport;

use super::BlockingMode;

use super::{Camera, MovementConfig, PanTiltPosition};

// Blocking mode implementation is always available
impl<P, T> Camera<BlockingMode, P, T>
where
    P: Profile + ProfileMetadata,
    T: BlockingTransport,
{
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
    pub fn wait_for_movement(&self, config: &MovementConfig) -> Result<(), Error> {
        // In blocking mode, we can't use event-driven detection with async channels
        // Users should use the async wait_for_movement method for event-driven detection

        // Use state querying (works in both blocking and async modes)
        if config.debug {
            log::debug!("Using state-query movement detection");
        }

        self.wait_using_state_query(config)
    }

    /// Wait for movement using state querying (fallback method).
    fn wait_using_state_query(&self, config: &MovementConfig) -> Result<(), Error> {
        let start = Instant::now();

        // Keep checking if camera is moving
        loop {
            if start.elapsed() > config.timeout {
                if config.debug {
                    log::debug!("Movement detection timed out");
                }
                return Err(Error::Timeout);
            }

            if !self.is_moving()? {
                if config.debug {
                    log::debug!("Movement completed (camera is idle)");
                }
                return Ok(());
            }

            // Yield to scheduler instead of sleeping
            std::thread::yield_now();
        }
    }

    /// Check if the camera is currently moving.
    ///
    /// This checks pan/tilt, zoom, and focus positions to detect movement.
    pub fn is_moving(&self) -> Result<bool, Error> {
        // Get first reading
        let pos1_pt = self.pan_tilt_position_inquiry()?;
        let pos1_zoom = self.zoom_position_inquiry()?;
        let pos1_focus = self.focus_position_inquiry()?;

        // Yield to scheduler
        std::thread::yield_now();

        // Get second reading
        let pos2_pt = self.pan_tilt_position_inquiry()?;
        let pos2_zoom = self.zoom_position_inquiry()?;
        let pos2_focus = self.focus_position_inquiry()?;

        // Check for movement with reasonable tolerances
        let pt_moving = !positions_equal_within_tolerance(
            PanTiltPosition {
                pan: pos1_pt.0.value(),
                tilt: pos1_pt.1.value(),
            },
            PanTiltPosition {
                pan: pos2_pt.0.value(),
                tilt: pos2_pt.1.value(),
            },
            2, // 2 units tolerance for pan/tilt
        );

        let zoom_moving = (pos1_zoom.value() as i32 - pos2_zoom.value() as i32).abs() > 10; // 10 units tolerance for zoom
        let focus_moving = (pos1_focus.value() as i32 - pos2_focus.value() as i32).abs() > 5; // 5 units tolerance for focus

        Ok(pt_moving || zoom_moving || focus_moving)
    }
}

#[cfg(feature = "async")]
use super::AsyncMode;
#[cfg(feature = "async")]
use crate::transport::AsyncTransport;

#[cfg(feature = "async")]
impl<P, T> Camera<AsyncMode, P, T>
where
    P: Profile + ProfileMetadata,
    T: AsyncTransport + 'static,
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
                log::debug!(
                    "Using event-driven movement detection (camera supports completion messages)"
                );
            }

            // Try to wait for completion message
            match self.wait_for_completion(config.timeout).await {
                Ok(()) => {
                    if config.debug {
                        log::debug!("Movement completed (received operation complete message)");
                    }
                    return Ok(());
                }
                Err(Error::Unsupported) => {
                    // Transport doesn't support waiting for completion
                    if config.debug {
                        log::debug!("Transport doesn't support event-driven detection, falling back to state query");
                    }
                }
                Err(Error::Timeout) => {
                    // No completion message within timeout
                    if config.debug {
                        log::debug!("No completion message received, falling back to state query");
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
            log::debug!("Using state-query movement detection");
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
                    log::debug!("Movement detection timed out");
                }
                return Err(Error::Timeout);
            }

            if !self.is_moving_async().await? {
                if config.debug {
                    log::debug!("Movement completed (camera is idle)");
                }
                return Ok(());
            }

            // Yield to scheduler using runtime abstraction
            let runtime = self.runtime().ok_or(Error::MissingRuntime)?;
            runtime.sleep(std::time::Duration::from_millis(1)).await;
        }
    }

    /// Check if the camera is currently moving (async version).
    pub async fn is_moving_async(&self) -> Result<bool, Error> {
        // Get first reading
        let pos1_pt = self.pan_tilt_position_inquiry().await?;
        let pos1_zoom = self.zoom_position_inquiry().await?;
        let pos1_focus = self.focus_position_inquiry().await?;

        // Yield to scheduler using runtime abstraction
        let runtime = self.runtime().ok_or(Error::MissingRuntime)?;
        runtime.sleep(std::time::Duration::from_millis(1)).await;

        // Get second reading
        let pos2_pt = self.pan_tilt_position_inquiry().await?;
        let pos2_zoom = self.zoom_position_inquiry().await?;
        let pos2_focus = self.focus_position_inquiry().await?;

        // Check for movement with reasonable tolerances
        let pt_moving = !positions_equal_within_tolerance(
            PanTiltPosition {
                pan: pos1_pt.0.value(),
                tilt: pos1_pt.1.value(),
            },
            PanTiltPosition {
                pan: pos2_pt.0.value(),
                tilt: pos2_pt.1.value(),
            },
            2, // 2 units tolerance for pan/tilt
        );

        let zoom_moving = (pos1_zoom.value() as i32 - pos2_zoom.value() as i32).abs() > 10; // 10 units tolerance for zoom
        let focus_moving = (pos1_focus.value() as i32 - pos2_focus.value() as i32).abs() > 5; // 5 units tolerance for focus

        Ok(pt_moving || zoom_moving || focus_moving)
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
