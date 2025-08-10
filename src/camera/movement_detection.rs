//! Event-driven movement detection for PTZ cameras.
//!
//! This module provides efficient movement detection using VISCA completion
//! messages when available, with automatic fallback to state querying.

use std::time::Instant;

use crate::{
    capabilities::{Profile, ProfileMetadata},
    error::Error,
    transport::Transport,
};

use super::{Camera, MovementConfig, PanTiltPosition};

impl<P: Profile + ProfileMetadata, T: Transport + Send + Sync + 'static> Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
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
        // Event-driven detection only works with async features (needs socket manager)
        #[cfg(feature = "async")]
        if P::SUPPORTS_OPERATION_COMPLETE {
            if config.debug {
                log::debug!("Camera supports completion messages, trying event-driven detection");
            }

            // Try to wait for completion message
            match self.wait_for_completion_blocking(config.timeout) {
                Ok(()) => {
                    if config.debug {
                        log::debug!("Movement completed (received operation complete message)");
                    }
                    return Ok(());
                }
                Err(Error::Unsupported) => {
                    // No socket manager or transport doesn't support it
                    if config.debug {
                        log::debug!(
                            "Event-driven detection not available, falling back to state query"
                        );
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
        use crate::camera::methods::inquiry::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking};

        // Get first reading
        let pos1_pt = self.get_pan_tilt_position()?;
        let pos1_zoom = self.get_zoom_position()?;
        let pos1_focus = self.get_focus_position()?;

        // Yield to scheduler
        std::thread::yield_now();

        // Get second reading
        let pos2_pt = self.get_pan_tilt_position()?;
        let pos2_zoom = self.get_zoom_position()?;
        let pos2_focus = self.get_focus_position()?;

        // Check for movement with reasonable tolerances
        let pt_moving = !positions_equal_within_tolerance(
            PanTiltPosition {
                pan: pos1_pt.0,
                tilt: pos1_pt.1,
            },
            PanTiltPosition {
                pan: pos2_pt.0,
                tilt: pos2_pt.1,
            },
            2, // 2 units tolerance for pan/tilt
        );

        let zoom_moving = (pos1_zoom as i32 - pos2_zoom as i32).abs() > 10; // 10 units tolerance for zoom
        let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5; // 5 units tolerance for focus

        Ok(pt_moving || zoom_moving || focus_moving)
    }
}

#[cfg(feature = "async")]
impl<P: Profile + ProfileMetadata, T: Transport + Send + Sync + 'static> Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
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

            // Yield to scheduler
            #[cfg(feature = "tokio")]
            tokio::task::yield_now().await;
            #[cfg(not(feature = "tokio"))]
            std::thread::yield_now();
        }
    }

    /// Check if the camera is currently moving (async version).
    pub async fn is_moving_async(&self) -> Result<bool, Error> {
        use crate::camera::methods::inquiry::{InquiryOps, PanTiltInquiryOps};

        // Get first reading
        let pos1_pt = self.get_pan_tilt_position().await?;
        let pos1_zoom = self.get_zoom_position().await?;
        let pos1_focus = self.get_focus_position().await?;

        // Yield to scheduler
        #[cfg(feature = "tokio")]
        tokio::task::yield_now().await;
        #[cfg(not(feature = "tokio"))]
        std::thread::yield_now();

        // Get second reading
        let pos2_pt = self.get_pan_tilt_position().await?;
        let pos2_zoom = self.get_zoom_position().await?;
        let pos2_focus = self.get_focus_position().await?;

        // Check for movement with reasonable tolerances
        let pt_moving = !positions_equal_within_tolerance(
            PanTiltPosition {
                pan: pos1_pt.0,
                tilt: pos1_pt.1,
            },
            PanTiltPosition {
                pan: pos2_pt.0,
                tilt: pos2_pt.1,
            },
            2, // 2 units tolerance for pan/tilt
        );

        let zoom_moving = (pos1_zoom as i32 - pos2_zoom as i32).abs() > 10; // 10 units tolerance for zoom
        let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5; // 5 units tolerance for focus

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
