//! Helper methods for intelligent camera movement completion detection.
//!
//! This module provides traits that add movement detection capabilities to cameras.
//! The implementation uses GAT-based probes to eliminate code duplication between
//! blocking and async variants.

use std::time::Duration;

use crate::{
    camera::Camera, capabilities::Profile, error::Error, transport::UnifiedTransport,
    units::Degrees,
};

// Re-export the config from movement_probe
pub use super::movement_probe::MovementDetectionConfig;

// Import the unified algorithms
use super::movement_detection::{
    wait_for_focus_completion, wait_for_pan_tilt_completion, wait_for_zoom_completion,
};

// Import the probe implementations
use super::probes::{BlockingFocusProbe, BlockingPanTiltProbe, BlockingZoomProbe};

#[cfg(feature = "tokio")]
use super::probes::{TokioFocusProbe, TokioPanTiltProbe, TokioZoomProbe};

/// Helper methods for camera movement operations (blocking).
pub trait MovementHelpers: Sized {
    /// Wait for pan/tilt movement to complete with default config.
    fn wait_for_pan_tilt_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_pan_tilt_completion_with_config(&config)
    }

    /// Wait for pan/tilt movement to complete with custom config.
    fn wait_for_pan_tilt_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for zoom movement to complete with default config.
    fn wait_for_zoom_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_zoom_completion_with_config(&config)
    }

    /// Wait for zoom movement to complete with custom config.
    fn wait_for_zoom_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for focus movement to complete with default config.
    fn wait_for_focus_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_focus_completion_with_config(&config)
    }

    /// Wait for focus movement to complete with custom config.
    fn wait_for_focus_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for all movements to complete.
    fn wait_for_all_movements(&self, timeout: Duration) -> Result<(), Error>;

    /// Move to a position and wait for completion.
    fn move_to_position_and_wait(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: Duration,
    ) -> Result<(), Error>;

    /// Check if the camera is currently moving.
    fn is_moving(&self) -> Result<bool, Error>;
}

/// Async helper methods for camera movement operations.
#[cfg(feature = "async")]
pub trait MovementHelpersAsync: Sized {
    /// Wait for pan/tilt movement to complete with default config.
    async fn wait_for_pan_tilt_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_pan_tilt_completion_with_config(&config).await
    }

    /// Wait for pan/tilt movement to complete with custom config.
    async fn wait_for_pan_tilt_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for zoom movement to complete with default config.
    async fn wait_for_zoom_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_zoom_completion_with_config(&config).await
    }

    /// Wait for zoom movement to complete with custom config.
    async fn wait_for_zoom_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for focus movement to complete with default config.
    async fn wait_for_focus_completion(&self, timeout: Duration) -> Result<(), Error> {
        let config = MovementDetectionConfig {
            timeout,
            ..Default::default()
        };
        self.wait_for_focus_completion_with_config(&config).await
    }

    /// Wait for focus movement to complete with custom config.
    async fn wait_for_focus_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error>;

    /// Wait for all movements to complete.
    async fn wait_for_all_movements(&self, timeout: Duration) -> Result<(), Error>;

    /// Move to a position and wait for completion.
    async fn move_to_position_and_wait(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: Duration,
    ) -> Result<(), Error>;

    /// Check if the camera is currently moving.
    async fn is_moving(&self) -> Result<bool, Error>;
}

// Blocking implementation
impl<P: Profile, T: UnifiedTransport> MovementHelpers for Camera<P, T> {
    fn wait_for_pan_tilt_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        let probe = BlockingPanTiltProbe::new(self);

        // Use the async algorithm but block on it
        futures::executor::block_on(wait_for_pan_tilt_completion(&probe, config))
    }

    fn wait_for_zoom_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        let probe = BlockingZoomProbe::new(self);
        futures::executor::block_on(wait_for_zoom_completion(&probe, config))
    }

    fn wait_for_focus_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        let probe = BlockingFocusProbe::new(self);
        futures::executor::block_on(wait_for_focus_completion(&probe, config))
    }

    fn wait_for_all_movements(&self, timeout: Duration) -> Result<(), Error> {
        use super::movement_probe::{
            positions_equal_within_tolerance, zoom_equal_within_tolerance, PanTiltPosition,
        };
        use crate::camera::methods::inquiry::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking};

        let start = Instant::now();
        let check_interval = Duration::from_millis(200);

        std::thread::sleep(Duration::from_millis(300));

        loop {
            if start.elapsed() > timeout {
                return Err(Error::Timeout);
            }

            // Check movement
            let pos1_pt = self.get_pan_tilt_position()?;
            let pos1_zoom = self.get_zoom_position()?;
            let pos1_focus = self.get_focus_position()?;

            std::thread::sleep(Duration::from_millis(50));

            let pos2_pt = self.get_pan_tilt_position()?;
            let pos2_zoom = self.get_zoom_position()?;
            let pos2_focus = self.get_focus_position()?;

            let pt_moving = !positions_equal_within_tolerance(
                PanTiltPosition {
                    pan: pos1_pt.0,
                    tilt: pos1_pt.1,
                },
                PanTiltPosition {
                    pan: pos2_pt.0,
                    tilt: pos2_pt.1,
                },
                2,
            );
            let zoom_moving = !zoom_equal_within_tolerance(pos1_zoom, pos2_zoom, 10);
            let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5;

            if !pt_moving && !zoom_moving && !focus_moving {
                std::thread::sleep(Duration::from_millis(100));

                // Double-check
                let pos3_pt = self.get_pan_tilt_position()?;
                let pos3_zoom = self.get_zoom_position()?;
                let pos3_focus = self.get_focus_position()?;

                let still_not_moving = positions_equal_within_tolerance(
                    PanTiltPosition {
                        pan: pos2_pt.0,
                        tilt: pos2_pt.1,
                    },
                    PanTiltPosition {
                        pan: pos3_pt.0,
                        tilt: pos3_pt.1,
                    },
                    2,
                ) && zoom_equal_within_tolerance(pos2_zoom, pos3_zoom, 10)
                    && (pos2_focus as i32 - pos3_focus as i32).abs() <= 5;

                if still_not_moving {
                    return Ok(());
                }
            }

            std::thread::sleep(check_interval);
        }
    }

    fn move_to_position_and_wait(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: Duration,
    ) -> Result<(), Error> {
        use crate::camera::methods::pan_tilt::PanTiltOpsBlocking;
        use crate::types::SpeedLevel;

        self.pan_tilt_absolute(pan, tilt, SpeedLevel::Fast)?;
        MovementHelpers::wait_for_pan_tilt_completion(self, timeout)
    }

    fn is_moving(&self) -> Result<bool, Error> {
        use super::movement_probe::{
            positions_equal_within_tolerance, zoom_equal_within_tolerance, PanTiltPosition,
        };
        use crate::camera::methods::inquiry::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking};

        let pos1_pt = self.get_pan_tilt_position()?;
        let pos1_zoom = self.get_zoom_position()?;
        let pos1_focus = self.get_focus_position()?;

        std::thread::sleep(Duration::from_millis(50));

        let pos2_pt = self.get_pan_tilt_position()?;
        let pos2_zoom = self.get_zoom_position()?;
        let pos2_focus = self.get_focus_position()?;

        let pt_moving = !positions_equal_within_tolerance(
            PanTiltPosition {
                pan: pos1_pt.0,
                tilt: pos1_pt.1,
            },
            PanTiltPosition {
                pan: pos2_pt.0,
                tilt: pos2_pt.1,
            },
            2,
        );
        let zoom_moving = !zoom_equal_within_tolerance(pos1_zoom, pos2_zoom, 10);
        let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5;

        Ok(pt_moving || zoom_moving || focus_moving)
    }
}

use std::time::Instant;

// Tokio async implementation
#[cfg(feature = "tokio")]
impl<P: Profile, T: UnifiedTransport> MovementHelpersAsync for Camera<P, T> {
    async fn wait_for_pan_tilt_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        let probe = TokioPanTiltProbe::new(self);
        wait_for_pan_tilt_completion(&probe, config).await
    }

    async fn wait_for_zoom_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        let probe = TokioZoomProbe::new(self);
        wait_for_zoom_completion(&probe, config).await
    }

    async fn wait_for_focus_completion_with_config(
        &self,
        config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        let probe = TokioFocusProbe::new(self);
        wait_for_focus_completion(&probe, config).await
    }

    async fn wait_for_all_movements(&self, timeout: Duration) -> Result<(), Error> {
        use super::movement_probe::{
            positions_equal_within_tolerance, zoom_equal_within_tolerance, PanTiltPosition,
        };
        use crate::camera::methods::inquiry::{InquiryOps, PanTiltInquiryOps};

        let start = Instant::now();
        let check_interval = Duration::from_millis(200);

        tokio::time::sleep(Duration::from_millis(300)).await;

        loop {
            if start.elapsed() > timeout {
                return Err(Error::Timeout);
            }

            // Check movement
            let pos1_pt = self.get_pan_tilt_position().await?;
            let pos1_zoom = self.get_zoom_position().await?;
            let pos1_focus = self.get_focus_position().await?;

            tokio::time::sleep(Duration::from_millis(50)).await;

            let pos2_pt = self.get_pan_tilt_position().await?;
            let pos2_zoom = self.get_zoom_position().await?;
            let pos2_focus = self.get_focus_position().await?;

            let pt_moving = !positions_equal_within_tolerance(
                PanTiltPosition {
                    pan: pos1_pt.0,
                    tilt: pos1_pt.1,
                },
                PanTiltPosition {
                    pan: pos2_pt.0,
                    tilt: pos2_pt.1,
                },
                2,
            );
            let zoom_moving = !zoom_equal_within_tolerance(pos1_zoom, pos2_zoom, 10);
            let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5;

            if !pt_moving && !zoom_moving && !focus_moving {
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Double-check
                let pos3_pt = self.get_pan_tilt_position().await?;
                let pos3_zoom = self.get_zoom_position().await?;
                let pos3_focus = self.get_focus_position().await?;

                let still_not_moving = positions_equal_within_tolerance(
                    PanTiltPosition {
                        pan: pos2_pt.0,
                        tilt: pos2_pt.1,
                    },
                    PanTiltPosition {
                        pan: pos3_pt.0,
                        tilt: pos3_pt.1,
                    },
                    2,
                ) && zoom_equal_within_tolerance(pos2_zoom, pos3_zoom, 10)
                    && (pos2_focus as i32 - pos3_focus as i32).abs() <= 5;

                if still_not_moving {
                    return Ok(());
                }
            }

            tokio::time::sleep(check_interval).await;
        }
    }

    async fn move_to_position_and_wait(
        &self,
        pan: Degrees,
        tilt: Degrees,
        timeout: Duration,
    ) -> Result<(), Error> {
        use crate::camera::methods::pan_tilt::PanTiltOps;
        use crate::types::SpeedLevel;

        self.pan_tilt_absolute(pan, tilt, SpeedLevel::Fast).await?;
        MovementHelpersAsync::wait_for_pan_tilt_completion(self, timeout).await
    }

    async fn is_moving(&self) -> Result<bool, Error> {
        use super::movement_probe::{
            positions_equal_within_tolerance, zoom_equal_within_tolerance, PanTiltPosition,
        };
        use crate::camera::methods::inquiry::{InquiryOps, PanTiltInquiryOps};

        let pos1_pt = self.get_pan_tilt_position().await?;
        let pos1_zoom = self.get_zoom_position().await?;
        let pos1_focus = self.get_focus_position().await?;

        tokio::time::sleep(Duration::from_millis(50)).await;

        let pos2_pt = self.get_pan_tilt_position().await?;
        let pos2_zoom = self.get_zoom_position().await?;
        let pos2_focus = self.get_focus_position().await?;

        let pt_moving = !positions_equal_within_tolerance(
            PanTiltPosition {
                pan: pos1_pt.0,
                tilt: pos1_pt.1,
            },
            PanTiltPosition {
                pan: pos2_pt.0,
                tilt: pos2_pt.1,
            },
            2,
        );
        let zoom_moving = !zoom_equal_within_tolerance(pos1_zoom, pos2_zoom, 10);
        let focus_moving = (pos1_focus as i32 - pos2_focus as i32).abs() > 5;

        Ok(pt_moving || zoom_moving || focus_moving)
    }
}

// Generic async implementation for non-tokio async runtimes
#[cfg(all(feature = "async", not(feature = "tokio")))]
impl<P: Profile, T: UnifiedTransport> MovementHelpersAsync for Camera<P, T> {
    async fn wait_for_pan_tilt_completion_with_config(
        &self,
        _config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Async movement helpers require tokio feature".into(),
        ))
    }

    async fn wait_for_zoom_completion_with_config(
        &self,
        _config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Async movement helpers require tokio feature".into(),
        ))
    }

    async fn wait_for_focus_completion_with_config(
        &self,
        _config: &MovementDetectionConfig,
    ) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Async movement helpers require tokio feature".into(),
        ))
    }

    async fn wait_for_all_movements(&self, _timeout: Duration) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Async movement helpers require tokio feature".into(),
        ))
    }

    async fn move_to_position_and_wait(
        &self,
        _pan: Degrees,
        _tilt: Degrees,
        _timeout: Duration,
    ) -> Result<(), Error> {
        Err(Error::InvalidState(
            "Async movement helpers require tokio feature".into(),
        ))
    }

    async fn is_moving(&self) -> Result<bool, Error> {
        Err(Error::InvalidState(
            "Async movement helpers require tokio feature".into(),
        ))
    }
}
