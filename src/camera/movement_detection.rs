//! Event-driven movement detection for Ptz cameras.
//!
//! This module provides efficient movement detection using VISCA completion
//! messages when available, with automatic fallback to state querying.

use std::time::{Duration, Instant};

#[cfg(not(feature = "async"))]
use crate::transport::BlockingTransport;
use crate::{
    capabilities::{Profile, ProfileMetadata},
    command::inquiry::{FocusPositionInquiry, PanTiltPositionInquiry, ZoomPositionInquiry},
    error::Error,
};
#[cfg(feature = "async")]
use crate::{executor::Executor, transport::AsyncTransport};

#[cfg(feature = "async")]
use super::AsyncCamera;
#[cfg(not(feature = "async"))]
use super::BlockingCamera;
use super::{MovementConfig, PanTiltPosition};

// Blocking mode implementation is only available without async feature
#[cfg(not(feature = "async"))]
impl<P, T> BlockingCamera<P, T>
where
    P: Profile + ProfileMetadata + Default,
    T: BlockingTransport,
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
        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only pan/tilt movement
            let pos1_response = self.send_command(&PanTiltPositionInquiry)?;
            let (pos1_pan, pos1_tilt) = match pos1_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::PanTiltPosition { pan, tilt },
                ) => (pan, tilt),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            };

            std::thread::sleep(Duration::from_millis(50));

            let pos2_response = self.send_command(&PanTiltPositionInquiry)?;
            let (pos2_pan, pos2_tilt) = match pos2_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::PanTiltPosition { pan, tilt },
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
            let pos1_response = self.send_command(&ZoomPositionInquiry)?;
            let pos1_zoom = match pos1_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::ZoomPosition { position },
                ) => position,
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            };

            std::thread::sleep(Duration::from_millis(50));

            let pos2_response = self.send_command(&ZoomPositionInquiry)?;
            let pos2_zoom = match pos2_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::ZoomPosition { position },
                ) => position,
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
            let pos1_response = self.send_command(&FocusPositionInquiry)?;
            let pos1_focus = match pos1_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::FocusPosition { position },
                ) => position,
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            };

            std::thread::sleep(Duration::from_millis(50));

            let pos2_response = self.send_command(&FocusPositionInquiry)?;
            let pos2_focus = match pos2_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::FocusPosition { position },
                ) => position,
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
        let pos1_pt_response = self.send_command(&PanTiltPositionInquiry)?;
        let (pos1_pan, pos1_tilt) = match pos1_pt_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::PanTiltPosition { pan, tilt },
            ) => (pan, tilt),
            _ => {
                return Err(Error::ParseError(
                    "Expected PanTiltPosition response".into(),
                ))
            }
        };

        let pos1_zoom_response = self.send_command(&ZoomPositionInquiry)?;
        let pos1_zoom = match pos1_zoom_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::ZoomPosition { position },
            ) => position,
            _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
        };

        let pos1_focus_response = self.send_command(&FocusPositionInquiry)?;
        let pos1_focus = match pos1_focus_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::FocusPosition { position },
            ) => position,
            _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
        };

        // Sleep briefly to allow state to change between samples
        std::thread::sleep(Duration::from_millis(1));

        // Get second reading
        let pos2_pt_response = self.send_command(&PanTiltPositionInquiry)?;
        let (pos2_pan, pos2_tilt) = match pos2_pt_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::PanTiltPosition { pan, tilt },
            ) => (pan, tilt),
            _ => {
                return Err(Error::ParseError(
                    "Expected PanTiltPosition response".into(),
                ))
            }
        };

        let pos2_zoom_response = self.send_command(&ZoomPositionInquiry)?;
        let pos2_zoom = match pos2_zoom_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::ZoomPosition { position },
            ) => position,
            _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
        };

        let pos2_focus_response = self.send_command(&FocusPositionInquiry)?;
        let pos2_focus = match pos2_focus_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::FocusPosition { position },
            ) => position,
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

#[cfg(feature = "async")]
impl<P, T, E> AsyncCamera<P, T, E>
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
                Err(Error::Unsupported) => {
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
            let executor = self.executor();
            executor.sleep(Duration::from_millis(5)).await;
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
        let executor = self.executor();

        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only pan/tilt movement
            let pos1_response = self.send_command(&PanTiltPositionInquiry).await?;
            let (pos1_pan, pos1_tilt) = match pos1_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::PanTiltPosition { pan, tilt },
                ) => (pan, tilt),
                _ => {
                    return Err(Error::ParseError(
                        "Expected PanTiltPosition response".into(),
                    ))
                }
            };

            executor.sleep(Duration::from_millis(50)).await;

            let pos2_response = self.send_command(&PanTiltPositionInquiry).await?;
            let (pos2_pan, pos2_tilt) = match pos2_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::PanTiltPosition { pan, tilt },
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
        let executor = self.executor();

        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only zoom movement
            let pos1_response = self.send_command(&ZoomPositionInquiry).await?;
            let pos1_zoom = match pos1_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::ZoomPosition { position },
                ) => position,
                _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
            };

            executor.sleep(Duration::from_millis(50)).await;

            let pos2_response = self.send_command(&ZoomPositionInquiry).await?;
            let pos2_zoom = match pos2_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::ZoomPosition { position },
                ) => position,
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
        let executor = self.executor();

        loop {
            if start.elapsed() > config.timeout {
                return Err(Error::Timeout);
            }

            // Check only focus movement
            let pos1_response = self.send_command(&FocusPositionInquiry).await?;
            let pos1_focus = match pos1_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::FocusPosition { position },
                ) => position,
                _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
            };

            executor.sleep(Duration::from_millis(50)).await;

            let pos2_response = self.send_command(&FocusPositionInquiry).await?;
            let pos2_focus = match pos2_response {
                crate::command::ViscaResponse::Inquiry(
                    crate::command::InquiryResponse::FocusPosition { position },
                ) => position,
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
            // For cameras that support operation complete messages,
            // we can use the runtime's completion listener
            // This will be implemented when we have the runtime event system
            return Err(Error::InvalidState(
                "Event-driven movement detection not yet implemented".into(),
            ));
        }

        // Fall back to state querying for all other cameras
        if config.debug {
            tracing::debug!("Using state-query movement detection (fallback mode)");
        }

        let start = Instant::now();
        let executor = self.executor();

        // Loop until timeout checking if camera has stopped moving
        while start.elapsed() < config.timeout {
            // Check if we're still moving
            let moving = self.is_moving_async().await?;
            if !moving {
                return Ok(());
            }

            // Wait a bit before checking again
            executor.sleep(Duration::from_millis(100)).await;
        }

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
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::PanTiltPosition { pan, tilt },
            ) => (pan, tilt),
            _ => {
                return Err(Error::ParseError(
                    "Expected PanTiltPosition response".into(),
                ))
            }
        };

        let pos1_zoom_response = self.send_command(&ZoomPositionInquiry).await?;
        let pos1_zoom = match pos1_zoom_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::ZoomPosition { position },
            ) => position,
            _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
        };

        let pos1_focus_response = self.send_command(&FocusPositionInquiry).await?;
        let pos1_focus = match pos1_focus_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::FocusPosition { position },
            ) => position,
            _ => return Err(Error::ParseError("Expected FocusPosition response".into())),
        };

        // Yield to scheduler using executor
        let executor = self.executor();
        executor.sleep(Duration::from_millis(1)).await;

        // Get second reading
        let pos2_pt_response = self.send_command(&PanTiltPositionInquiry).await?;
        let (pos2_pan, pos2_tilt) = match pos2_pt_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::PanTiltPosition { pan, tilt },
            ) => (pan, tilt),
            _ => {
                return Err(Error::ParseError(
                    "Expected PanTiltPosition response".into(),
                ))
            }
        };

        let pos2_zoom_response = self.send_command(&ZoomPositionInquiry).await?;
        let pos2_zoom = match pos2_zoom_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::ZoomPosition { position },
            ) => position,
            _ => return Err(Error::ParseError("Expected ZoomPosition response".into())),
        };

        let pos2_focus_response = self.send_command(&FocusPositionInquiry).await?;
        let pos2_focus = match pos2_focus_response {
            crate::command::ViscaResponse::Inquiry(
                crate::command::InquiryResponse::FocusPosition { position },
            ) => position,
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

/// Check if two positions are equal within tolerance.
#[inline]
fn positions_equal_within_tolerance(
    pos1: PanTiltPosition,
    pos2: PanTiltPosition,
    tolerance: i16,
) -> bool {
    (pos1.pan - pos2.pan).abs() <= tolerance && (pos1.tilt - pos2.tilt).abs() <= tolerance
}
