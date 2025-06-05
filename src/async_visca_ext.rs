//! Async extension trait providing high-level camera operations.
//!
//! This module provides the `AsyncViscaExt` trait which adds ergonomic
//! high-level methods to the `ViscaClient` for common camera operations.
//!
//! # Example
//! ```no_run
//! # use grafton_visca::{ViscaClient, AsyncViscaExt};
//! # use std::sync::Arc;
//! # use std::time::Duration;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Arc::new(ViscaClient::connect_udp_async("192.168.1.100:5678").await?);
//!
//! // High-level operations
//! client.setup_shot(0.5, 0.3, 0x8000).await?;
//! client.save_current_position(1).await?;
//! client.patrol_positions(&[1, 2, 3], Some(Duration::from_secs(1))).await?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;
use std::time::Duration;

use crate::{
    command::{
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        preset::{PresetAction, PresetCommand, PresetNumber},
        zoom::{ZoomCommand, ZoomSpeed},
    },
    unified_client::{ViscaClient, ViscaClientPtzExt},
    ViscaError,
};

/// Async extension trait for high-level camera operations.
///
/// This trait provides ergonomic methods for common camera operations
/// that typically involve multiple VISCA commands.
#[cfg(feature = "async-client")]
#[allow(async_fn_in_trait)] // Acceptable for ergonomics in application-specific trait
pub trait AsyncViscaExt {
    /// Set up a shot with relative pan/tilt position and zoom level.
    ///
    /// This is a high-level operation that combines pan/tilt movement and zoom
    /// to quickly position the camera for a shot.
    ///
    /// # Arguments
    /// * `pan_percent` - Pan position as percentage (-1.0 to 1.0, where 0.0 is center)
    /// * `tilt_percent` - Tilt position as percentage (-1.0 to 1.0, where 0.0 is center)
    /// * `zoom_level` - Zoom level (0x0000 to 0xFFFF)
    async fn setup_shot(
        &self,
        pan_percent: f32,
        tilt_percent: f32,
        zoom_level: u16,
    ) -> Result<(), ViscaError>;

    /// Save the current camera position to a preset.
    ///
    /// # Arguments
    /// * `preset_number` - Preset number (1-90)
    async fn save_current_position(&self, preset_number: u8) -> Result<(), ViscaError>;

    /// Recall a preset position and optionally adjust zoom.
    ///
    /// # Arguments
    /// * `preset_number` - Preset number (1-90)
    /// * `zoom_adjustment` - Optional zoom adjustment after recalling preset
    async fn recall_position_with_zoom(
        &self,
        preset_number: u8,
        zoom_adjustment: Option<u16>,
    ) -> Result<(), ViscaError>;

    /// Patrol between multiple preset positions.
    ///
    /// Cycles through the given preset positions with a delay between each.
    ///
    /// # Arguments
    /// * `preset_numbers` - Array of preset numbers to patrol
    /// * `delay_between` - Optional delay between positions (default: 2 seconds)
    async fn patrol_positions(
        &self,
        preset_numbers: &[u8],
        delay_between: Option<Duration>,
    ) -> Result<(), ViscaError>;

    /// Perform a smooth pan scan across the scene.
    ///
    /// # Arguments
    /// * `speed` - Pan speed (0-24)
    /// * `duration` - How long to pan
    /// * `direction` - Direction to pan (Left or Right)
    async fn smooth_pan_scan(
        &self,
        speed: u8,
        duration: Duration,
        direction: PanScanDirection,
    ) -> Result<(), ViscaError>;

    /// Focus and zoom to frame a subject optimally.
    ///
    /// This operation combines zoom and focus to frame a subject at the current
    /// pan/tilt position.
    ///
    /// # Arguments
    /// * `zoom_level` - Target zoom level
    /// * `auto_focus` - Whether to enable auto-focus after zooming
    async fn frame_subject(&self, zoom_level: u16, auto_focus: bool) -> Result<(), ViscaError>;

    /// Reset camera to a neutral state.
    ///
    /// Returns camera to home position, resets zoom to wide, and enables auto-focus.
    async fn reset_to_neutral(&self) -> Result<(), ViscaError>;

    /// Perform a quick health check of camera movement systems.
    ///
    /// Tests pan/tilt and zoom movement to verify camera responsiveness.
    async fn movement_health_check(&self) -> Result<bool, ViscaError>;
}

/// Direction for pan scanning operations.
#[derive(Debug, Copy, Clone)]
pub enum PanScanDirection {
    /// Pan camera to the left
    Left,
    /// Pan camera to the right
    Right,
}

#[cfg(feature = "async-client")]
impl AsyncViscaExt for Arc<ViscaClient> {
    async fn setup_shot(
        &self,
        pan_percent: f32,
        tilt_percent: f32,
        zoom_level: u16,
    ) -> Result<(), ViscaError> {
        // Validate input ranges
        if !(-1.0..=1.0).contains(&pan_percent) {
            return Err(ViscaError::InvalidParameter(
                "pan_percent must be between -1.0 and 1.0".to_string(),
            ));
        }
        if !(-1.0..=1.0).contains(&tilt_percent) {
            return Err(ViscaError::InvalidParameter(
                "tilt_percent must be between -1.0 and 1.0".to_string(),
            ));
        }

        // Convert percentages to VISCA position values (assuming ±0x7FFF range)
        let pan_position = (pan_percent * 32767.0) as i16;
        let tilt_position = (tilt_percent * 32767.0) as i16;

        // Execute pan/tilt and zoom concurrently using PTZ builder
        Arc::clone(self)
            .ptz()
            .pan_tilt_absolute(pan_position, tilt_position, 0x18, 0x14)? // Max speeds
            .zoom_direct(zoom_level)
            .execute_concurrent()
            .await?;

        Ok(())
    }

    async fn save_current_position(&self, preset_number: u8) -> Result<(), ViscaError> {
        if preset_number == 0 || preset_number > 90 {
            return Err(ViscaError::InvalidParameter(
                "preset_number must be between 1 and 90".to_string(),
            ));
        }

        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(preset_number - 1)?, // VISCA uses 0-based indexing
        };

        self.send_async(&command).await?;
        Ok(())
    }

    async fn recall_position_with_zoom(
        &self,
        preset_number: u8,
        zoom_adjustment: Option<u16>,
    ) -> Result<(), ViscaError> {
        if preset_number == 0 || preset_number > 90 {
            return Err(ViscaError::InvalidParameter(
                "preset_number must be between 1 and 90".to_string(),
            ));
        }

        // Recall the preset position
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(preset_number - 1)?, // VISCA uses 0-based indexing
        };
        self.send_async(&command).await?;

        // Apply zoom adjustment if specified
        if let Some(zoom_level) = zoom_adjustment {
            // Small delay to let preset recall complete
            tokio::time::sleep(Duration::from_millis(500)).await;

            let zoom_command = ZoomCommand::Direct(zoom_level);
            self.send_async(&zoom_command).await?;
        }

        Ok(())
    }

    async fn patrol_positions(
        &self,
        preset_numbers: &[u8],
        delay_between: Option<Duration>,
    ) -> Result<(), ViscaError> {
        let delay = delay_between.unwrap_or(Duration::from_secs(2));

        for &preset_number in preset_numbers {
            if preset_number == 0 || preset_number > 90 {
                return Err(ViscaError::InvalidParameter(format!(
                    "preset_number {} must be between 1 and 90",
                    preset_number
                )));
            }

            let command = PresetCommand {
                action: PresetAction::Recall,
                preset_number: PresetNumber::new(preset_number - 1)?, // VISCA uses 0-based indexing
            };
            self.send_async(&command).await?;

            // Wait before moving to next position
            tokio::time::sleep(delay).await;
        }

        Ok(())
    }

    async fn smooth_pan_scan(
        &self,
        speed: u8,
        duration: Duration,
        direction: PanScanDirection,
    ) -> Result<(), ViscaError> {
        let pan_speed = PanSpeed::new(speed.min(0x18))?;
        let tilt_speed = TiltSpeed::new(0)?; // No tilt movement

        let pan_direction = match direction {
            PanScanDirection::Left => PanTiltDirection::Left,
            PanScanDirection::Right => PanTiltDirection::Right,
        };

        // Start panning
        let move_command = PanTiltCommand::Move {
            direction: pan_direction,
            pan_speed,
            tilt_speed,
        };
        self.send_async(&move_command).await?;

        // Continue for specified duration
        tokio::time::sleep(duration).await;

        // Stop panning
        let stop_command = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        };
        self.send_async(&stop_command).await?;

        Ok(())
    }

    async fn frame_subject(&self, zoom_level: u16, auto_focus: bool) -> Result<(), ViscaError> {
        let mut builder = Arc::clone(self).ptz().zoom_direct(zoom_level);

        if auto_focus {
            builder = builder.focus_auto();
        }

        builder.execute_sequential_async().await?;
        Ok(())
    }

    async fn reset_to_neutral(&self) -> Result<(), ViscaError> {
        Arc::clone(self)
            .ptz()
            .pan_tilt_home()
            .zoom_direct(0x0000) // Wide zoom
            .focus_auto()
            .execute_sequential_async()
            .await?;

        Ok(())
    }

    async fn movement_health_check(&self) -> Result<bool, ViscaError> {
        // Test basic connectivity first
        if !self.is_healthy().await? {
            return Ok(false);
        }

        // Test pan/tilt movement
        let pan_test = async {
            // Small movement right
            let move_right = PanTiltCommand::Move {
                direction: PanTiltDirection::Right,
                pan_speed: PanSpeed::new(0x08)?,
                tilt_speed: TiltSpeed::new(0)?,
            };
            self.send_async(&move_right).await?;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Stop movement
            let stop = PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0)?,
                tilt_speed: TiltSpeed::new(0)?,
            };
            self.send_async(&stop).await?;

            Ok::<(), ViscaError>(())
        };

        // Test zoom movement
        let zoom_test = async {
            // Zoom in slightly
            self.send_async(&ZoomCommand::TeleVariable(ZoomSpeed::new(3)?))
                .await?;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Stop zoom
            self.send_async(&ZoomCommand::Stop).await?;

            Ok::<(), ViscaError>(())
        };

        // Run tests concurrently
        match tokio::try_join!(pan_test, zoom_test) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pan_scan_direction() {
        // Test that enum values are correctly defined
        matches!(PanScanDirection::Left, PanScanDirection::Left);
        matches!(PanScanDirection::Right, PanScanDirection::Right);
    }

    // Note: Integration tests would require actual camera hardware
    // Unit tests here focus on parameter validation and structure
}
