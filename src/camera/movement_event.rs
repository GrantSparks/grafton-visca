//! Event-driven movement detection using VISCA operation complete messages.
//!
//! This module provides an efficient alternative to polling-based movement detection
//! for cameras that support VISCA operation complete (0x51) messages. Instead of
//! continuously polling camera position, it waits for the camera to signal completion.

use crate::{capabilities::ProfileMetadata, command::Response, error::Error};
use std::time::Duration;

/// Configuration for event-driven movement detection.
///
/// This is simpler than polling-based detection since we rely on camera events.
#[derive(Debug, Clone, Copy)]
pub struct EventDrivenConfig {
    /// Maximum time to wait for operation complete message.
    /// Default: 30 seconds (should match camera's COMPLETION_TIMEOUT).
    pub timeout: Duration,

    /// Whether to enable debug logging.
    pub debug: bool,

    /// Fallback to polling if completion message not received within this time.
    /// Some cameras might not send completion for very small movements.
    /// Default: 2 seconds.
    pub fallback_timeout: Duration,
}

impl Default for EventDrivenConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            debug: false,
            fallback_timeout: Duration::from_secs(2),
        }
    }
}

/// Event-driven movement detection for cameras that support operation complete.
///
/// This trait provides methods that wait for the camera's completion messages
/// instead of polling position. This is more efficient and accurate than polling.
pub trait EventDrivenMovement {
    /// Check if this camera supports event-driven movement detection.
    fn supports_event_driven(&self) -> bool;

    /// Wait for a movement operation to complete using event messages.
    ///
    /// Returns Ok(true) if completion message received, Ok(false) if fallback used.
    fn wait_for_operation_complete(&self, config: &EventDrivenConfig) -> Result<bool, Error>;
}

#[cfg(feature = "async")]
/// Async version of event-driven movement detection.
pub trait EventDrivenMovementAsync {
    /// Check if this camera supports event-driven movement detection.
    async fn supports_event_driven(&self) -> bool;

    /// Wait for a movement operation to complete using event messages.
    ///
    /// Returns Ok(true) if completion message received, Ok(false) if fallback used.
    async fn wait_for_operation_complete(&self, config: &EventDrivenConfig) -> Result<bool, Error>;
}

// Helper function to check if a response is an operation complete message
#[allow(dead_code)]
pub(crate) fn is_operation_complete(response: &Response) -> bool {
    matches!(response, Response::Completion)
}

/// Smart movement detection that chooses the best strategy.
///
/// This function automatically selects between event-driven and polling-based
/// detection based on camera capabilities and configuration.
///
/// Note: This function requires access to the camera instance for event-driven detection.
/// The current signature doesn't provide that, so it always falls back to polling.
/// Use `smart_wait_for_movement_with_camera` for full event-driven support.
#[cfg(feature = "async")]
pub async fn smart_wait_for_movement<P, M, Z, F>(
    _camera_profile: &P,
    movement_probe: &M,
    zoom_probe: &Z,
    focus_probe: &F,
    config: &super::movement_probe::MovementDetectionConfig,
) -> Result<(), Error>
where
    P: ProfileMetadata,
    M: super::movement_probe::MovementProbe,
    Z: super::movement_probe::ZoomProbe,
    F: super::movement_probe::FocusProbe,
{
    // Check if camera supports operation complete messages
    if P::SUPPORTS_OPERATION_COMPLETE {
        // Try event-driven approach first
        if config.debug {
            log::debug!(
                "Camera supports event-driven detection, but camera instance not available"
            );
            log::debug!("Use smart_wait_for_movement_with_camera for event-driven support");
        }
    }

    // Fall back to polling-based detection
    if config.debug {
        log::debug!("Using polling-based movement detection");
    }
    super::movement_detection::wait_for_all_movements(
        movement_probe,
        zoom_probe,
        focus_probe,
        config,
    )
    .await
}

/// Smart movement detection with camera instance for event-driven support.
///
/// This function automatically selects between event-driven and polling-based
/// detection based on camera capabilities and configuration.
#[cfg(feature = "async")]
pub async fn smart_wait_for_movement_with_camera<P, T, M, Z, F>(
    camera: &super::Camera<P, T>,
    movement_probe: &M,
    zoom_probe: &Z,
    focus_probe: &F,
    config: &super::movement_probe::MovementDetectionConfig,
) -> Result<(), Error>
where
    P: ProfileMetadata + crate::capabilities::Profile,
    T: crate::transport::UnifiedTransport,
    M: super::movement_probe::MovementProbe,
    Z: super::movement_probe::ZoomProbe,
    F: super::movement_probe::FocusProbe,
{
    // Check if camera supports operation complete messages
    if P::SUPPORTS_OPERATION_COMPLETE {
        // Try event-driven approach first
        if config.debug {
            log::debug!(
                "Using event-driven movement detection (camera supports operation complete)"
            );
        }

        // Create event config from movement config
        let event_config = EventDrivenConfig {
            timeout: config.timeout,
            debug: config.debug,
            fallback_timeout: config.no_movement_timeout,
        };

        // Try to wait for completion message
        match camera.wait_for_completion(event_config.timeout).await {
            Ok(()) => {
                if config.debug {
                    log::debug!("Movement completed (received operation complete message)");
                }
                return Ok(());
            }
            Err(Error::Unsupported) => {
                // Transport doesn't support waiting for completion
                if config.debug {
                    log::debug!(
                        "Transport doesn't support event-driven detection, falling back to polling"
                    );
                }
            }
            Err(Error::Timeout) => {
                // No completion message within fallback timeout, try polling
                if config.debug {
                    log::debug!(
                        "No completion message received within fallback timeout, using polling"
                    );
                }
            }
            Err(e) => {
                // Other error, propagate it
                return Err(e);
            }
        }
    }

    // Fall back to polling-based detection
    if config.debug {
        log::debug!("Using polling-based movement detection");
    }
    super::movement_detection::wait_for_all_movements(
        movement_probe,
        zoom_probe,
        focus_probe,
        config,
    )
    .await
}

/// Smart movement detection for blocking code.
pub fn smart_wait_for_movement_blocking<P, T>(
    camera: &super::Camera<P, T>,
    config: &super::movement_probe::MovementDetectionConfig,
) -> Result<(), Error>
where
    P: ProfileMetadata + crate::capabilities::Profile,
    T: crate::transport::UnifiedTransport,
{
    // Check if camera supports operation complete messages
    if P::SUPPORTS_OPERATION_COMPLETE {
        if config.debug {
            log::debug!(
                "Using event-driven movement detection (camera supports operation complete)"
            );
        }

        // Create event config from movement config
        let event_config = EventDrivenConfig {
            timeout: config.timeout,
            debug: config.debug,
            fallback_timeout: config.no_movement_timeout,
        };

        // Try to wait for completion message
        match camera.wait_for_completion_blocking(event_config.timeout) {
            Ok(()) => {
                if config.debug {
                    log::debug!("Movement completed (received operation complete message)");
                }
                return Ok(());
            }
            Err(Error::Unsupported) => {
                // Transport doesn't support waiting for completion
                if config.debug {
                    log::debug!(
                        "Transport doesn't support event-driven detection, falling back to polling"
                    );
                }
            }
            Err(Error::Timeout) => {
                // No completion message within fallback timeout, try polling
                if config.debug {
                    log::debug!(
                        "No completion message received within fallback timeout, using polling"
                    );
                }
            }
            Err(e) => {
                // Other error, propagate it
                return Err(e);
            }
        }
    }

    // Use polling-based detection
    if config.debug {
        log::debug!("Using polling-based movement detection");
    }
    super::movement_detection::wait_for_all_movements_blocking(camera, config)
}
