//! Async extension trait providing high-level camera operations.
//!
//! This module provides the `AsyncExt` trait which adds ergonomic
//! high-level methods to the `Client` for common camera operations.
//!
//! # Example
//! ```no_run
//! # #[cfg(feature = "async-client")]
//! # use grafton_visca::{Client, AsyncExt};
//! # #[cfg(feature = "async-client")]
//! # use std::sync::Arc;
//! # #[cfg(feature = "async-client")]
//! # use std::time::Duration;
//! # #[cfg(feature = "async-client")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = Arc::new(Client::connect_udp_async("192.168.1.100:5678").await?);
//! #
//! # // High-level operations
//! # client.setup_shot(0.5, 0.3, 0x8000).await?;
//! # client.save_current_position(1).await?;
//! # client.patrol_positions(&[1, 2, 3], Some(Duration::from_secs(1))).await?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "async-client"))]
//! # fn example() {}
//! ```

// Standard library imports
#[cfg(feature = "async-client")]
use std::sync::Arc;
#[cfg(feature = "async-client")]
use std::time::Duration;

// Third-party crate imports
// (none)

// Workspace / local-crate imports
#[cfg(feature = "async-client")]
use crate::{
    command::{
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        preset::{PresetAction, PresetCommand, PresetNumber},
        zoom::{ZoomCommand, ZoomSpeed},
    },
    error::Error,
    unified_client::{Client, ClientPtzExt},
};

/// Async extension trait for high-level camera operations.
///
/// This trait provides ergonomic methods for common camera operations
/// that typically involve multiple VISCA commands.
#[cfg(feature = "async-client")]
#[allow(clippy::manual_async_fn)] // Required to avoid async_fn_in_trait warning
pub trait AsyncExt {
    /// Set up a shot with relative pan/tilt position and zoom level.
    ///
    /// This is a high-level operation that combines pan/tilt movement and zoom
    /// to quickly position the camera for a shot.
    ///
    /// # Arguments
    /// * `pan_percent` - Pan position as percentage (-1.0 to 1.0, where 0.0 is center)
    /// * `tilt_percent` - Tilt position as percentage (-1.0 to 1.0, where 0.0 is center)
    /// * `zoom_level` - Zoom level (0x0000 to 0xFFFF)
    fn setup_shot(
        &self,
        pan_percent: f32,
        tilt_percent: f32,
        zoom_level: u16,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;

    /// Save the current camera position to a preset.
    ///
    /// # Arguments
    /// * `preset_number` - Preset number (1-90)
    fn save_current_position(
        &self,
        preset_number: u8,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;

    /// Recall a preset position and optionally adjust zoom.
    ///
    /// # Arguments
    /// * `preset_number` - Preset number (1-90)
    /// * `zoom_adjustment` - Optional zoom adjustment after recalling preset
    fn recall_position_with_zoom(
        &self,
        preset_number: u8,
        zoom_adjustment: Option<u16>,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;

    /// Patrol between multiple preset positions.
    ///
    /// Cycles through the given preset positions with a delay between each.
    ///
    /// # Arguments
    /// * `preset_numbers` - Array of preset numbers to patrol
    /// * `delay_between` - Optional delay between positions (default: 2 seconds)
    fn patrol_positions(
        &self,
        preset_numbers: &[u8],
        delay_between: Option<Duration>,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;

    /// Perform a smooth pan scan across the scene.
    ///
    /// # Arguments
    /// * `speed` - Pan speed (0-24)
    /// * `duration` - How long to pan
    /// * `direction` - Direction to pan (Left or Right)
    fn smooth_pan_scan(
        &self,
        speed: u8,
        duration: Duration,
        direction: PanScanDirection,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;

    /// Focus and zoom to frame a subject optimally.
    ///
    /// This operation combines zoom and focus to frame a subject at the current
    /// pan/tilt position.
    ///
    /// # Arguments
    /// * `zoom_level` - Target zoom level
    /// * `auto_focus` - Whether to enable auto-focus after zooming
    fn frame_subject(
        &self,
        zoom_level: u16,
        auto_focus: bool,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;

    /// Reset camera to a neutral state.
    ///
    /// Returns camera to home position, resets zoom to minimum, and enables auto-focus.
    fn reset_to_neutral(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send;

    /// Perform a quick health check of camera movement systems.
    ///
    /// Tests pan/tilt and zoom movement to verify camera responsiveness.
    fn movement_health_check(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send;
}

/// Direction for pan scanning operations.
#[cfg(feature = "async-client")]
#[derive(Debug, Copy, Clone)]
pub enum PanScanDirection {
    /// Pan camera to the left
    Left,
    /// Pan camera to the right
    Right,
}

#[cfg(feature = "async-client")]
#[allow(clippy::manual_async_fn)] // Required to avoid async_fn_in_trait warning
impl AsyncExt for Arc<Client> {
    fn setup_shot(
        &self,
        pan_percent: f32,
        tilt_percent: f32,
        zoom_level: u16,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
            // Validate input ranges
            if !(-1.0..=1.0).contains(&pan_percent) {
                return Err(Error::InvalidParameter(
                    "pan_percent must be between -1.0 and 1.0".to_string(),
                ));
            }
            if !(-1.0..=1.0).contains(&tilt_percent) {
                return Err(Error::InvalidParameter(
                    "tilt_percent must be between -1.0 and 1.0".to_string(),
                ));
            }

            // Convert percentages to VISCA position values (assuming ±0x7FFF range)
            #[allow(clippy::cast_possible_truncation)]
            let pan_position = (pan_percent * 32767.0).clamp(-32767.0, 32767.0) as i16;
            #[allow(clippy::cast_possible_truncation)]
            let tilt_position = (tilt_percent * 32767.0).clamp(-32767.0, 32767.0) as i16;

            // Execute pan/tilt and zoom concurrently using PTZ builder
            let _ = Self::clone(self)
                .ptz()
                .pan_tilt_absolute(pan_position, tilt_position, 0x18, 0x14)? // Max speeds
                .zoom_direct(zoom_level)
                .execute_concurrent()
                .await?;

            Ok(())
        }
    }

    fn save_current_position(
        &self,
        preset_number: u8,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
            if preset_number == 0 || preset_number > 90 {
                return Err(Error::InvalidParameter(
                    "preset_number must be between 1 and 90".to_string(),
                ));
            }

            let command = PresetCommand {
                action: PresetAction::Set,
                preset_number: PresetNumber::new(preset_number - 1)?, // VISCA uses 0-based indexing
            };

            let _ = self.send_async(&command).await?;
            Ok(())
        }
    }

    fn recall_position_with_zoom(
        &self,
        preset_number: u8,
        zoom_adjustment: Option<u16>,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
            if preset_number == 0 || preset_number > 90 {
                return Err(Error::InvalidParameter(
                    "preset_number must be between 1 and 90".to_string(),
                ));
            }

            // Recall the preset position
            let command = PresetCommand {
                action: PresetAction::Recall,
                preset_number: PresetNumber::new(preset_number - 1)?, // VISCA uses 0-based indexing
            };
            let _ = self.send_async(&command).await?;

            // Apply zoom adjustment if specified
            if let Some(zoom_level) = zoom_adjustment {
                // Small delay to let preset recall complete
                tokio::time::sleep(Duration::from_millis(500)).await;

                let zoom_command = ZoomCommand::Direct(zoom_level);
                let _ = self.send_async(&zoom_command).await?;
            }

            Ok(())
        }
    }

    fn patrol_positions(
        &self,
        preset_numbers: &[u8],
        delay_between: Option<Duration>,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
            let delay = delay_between.unwrap_or(Duration::from_secs(2));

            for &preset_number in preset_numbers {
                if preset_number == 0 || preset_number > 90 {
                    return Err(Error::InvalidParameter(format!(
                        "preset_number {preset_number} must be between 1 and 90"
                    )));
                }

                let command = PresetCommand {
                    action: PresetAction::Recall,
                    preset_number: PresetNumber::new(preset_number - 1)?, // VISCA uses 0-based indexing
                };
                let _ = self.send_async(&command).await?;

                // Wait before moving to next position
                tokio::time::sleep(delay).await;
            }

            Ok(())
        }
    }

    fn smooth_pan_scan(
        &self,
        speed: u8,
        duration: Duration,
        direction: PanScanDirection,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
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
            let _ = self.send_async(&move_command).await?;

            // Continue for specified duration
            tokio::time::sleep(duration).await;

            // Stop panning
            let stop_command = PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0)?,
                tilt_speed: TiltSpeed::new(0)?,
            };
            let _ = self.send_async(&stop_command).await?;

            Ok(())
        }
    }

    fn frame_subject(
        &self,
        zoom_level: u16,
        auto_focus: bool,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
            let mut builder = Self::clone(self).ptz().zoom_direct(zoom_level);

            if auto_focus {
                builder = builder.focus_auto();
            }

            let _ = builder.execute_sequential_async().await?;
            Ok(())
        }
    }

    fn reset_to_neutral(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
            let _ = Self::clone(self)
                .ptz()
                .pan_tilt_home()
                .zoom_direct(0x0000) // Minimum zoom
                .focus_auto()
                .execute_sequential_async()
                .await?;

            Ok(())
        }
    }

    fn movement_health_check(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send {
        async move {
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
                let _ = self.send_async(&move_right).await?;

                tokio::time::sleep(Duration::from_millis(200)).await;

                // Stop movement
                let stop = PanTiltCommand::Move {
                    direction: PanTiltDirection::Stop,
                    pan_speed: PanSpeed::new(0)?,
                    tilt_speed: TiltSpeed::new(0)?,
                };
                let _ = self.send_async(&stop).await?;

                Ok::<(), Error>(())
            };

            // Test zoom movement
            let zoom_test = async {
                // Zoom in slightly
                let _ = self
                    .send_async(&ZoomCommand::ZoomInVariable(ZoomSpeed::new(3)?))
                    .await?;
                tokio::time::sleep(Duration::from_millis(200)).await;

                // Stop zoom
                let _ = self.send_async(&ZoomCommand::Stop).await?;

                Ok::<(), Error>(())
            };

            // Run tests concurrently
            match tokio::try_join!(pan_test, zoom_test) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }
    }
}

#[cfg(all(test, feature = "async-client"))]
mod tests {
    use super::*;

    #[test]
    const fn test_pan_scan_direction() {
        // Test that enum values are correctly defined
        matches!(PanScanDirection::Left, PanScanDirection::Left);
        matches!(PanScanDirection::Right, PanScanDirection::Right);
    }

    // Note: Integration tests would require actual camera hardware
    // Unit tests here focus on parameter validation and structure
}
