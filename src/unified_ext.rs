//! Unified extension traits for VISCA camera control.
//!
//! This module provides a clean, unified API that works seamlessly with both
//! async and blocking contexts. The key design principle is to leverage the
//! existing `ViscaClient` which already handles async/sync unification internally.
//!
//! # Design Philosophy
//!
//! Rather than duplicating code or using complex macros, we provide:
//! - A single set of traits with clear, descriptive method names
//! - Methods that automatically work in the appropriate context
//! - No `_async` suffixes or duplicate trait definitions
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "blocking-client")]
//! # use grafton_visca::{ViscaClient, ViscaError, CameraExt};
//! # #[cfg(feature = "blocking-client")]
//! # fn example(camera: &ViscaClient) -> Result<(), ViscaError> {
//! // Works in both sync and async contexts!
//! if camera.is_powered_on()? {
//!     camera.zoom_to_position(0x4000)?;
//!     camera.save_current_as_preset(1)?;
//! }
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "blocking-client"))]
//! # fn main() {}
//! ```

use crate::{
    command::{
        focus::FocusSpeed,
        pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
        zoom::ZoomSpeed,
    },
    ViscaClient, ViscaError,
};

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::{
    command::{
        pan_tilt::PanTiltCommand,
        power::Power,
        preset::{PresetAction, PresetCommand, PresetNumber},
        zoom::ZoomCommand,
        InquiryCommand, PowerCommand,
    },
    ViscaInquiryResponse, ViscaResponse,
};

#[cfg(feature = "blocking-client")]
use crate::command::focus::FocusCommand;
use std::sync::Arc;
#[cfg(feature = "async-client")]
use std::time::Duration;

/// Main camera control extension trait.
///
/// This trait provides high-level camera control methods that work
/// transparently in both sync and async contexts.
pub trait CameraExt {
    // Power Control

    /// Check if the camera is powered on.
    ///
    /// # Errors
    /// Returns `ViscaError` if the power status cannot be queried.
    fn is_powered_on(&self) -> Result<bool, ViscaError>;

    /// Power on the camera.
    ///
    /// # Errors
    /// Returns `ViscaError` if the power command cannot be executed.
    fn power_on(&self) -> Result<(), ViscaError>;

    /// Power off the camera (standby mode).
    ///
    /// # Errors
    /// Returns `ViscaError` if the power command cannot be executed.
    fn power_off(&self) -> Result<(), ViscaError>;

    // Zoom Control

    /// Get the current zoom position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the zoom position cannot be queried.
    fn zoom_position(&self) -> Result<u16, ViscaError>;

    /// Zoom to a specific position (0x0000 to 0xFFFF).
    ///
    /// # Errors
    /// Returns `ViscaError` if the zoom command cannot be executed.
    fn zoom_to_position(&self, position: u16) -> Result<(), ViscaError>;

    /// Start zooming in at the specified speed.
    ///
    /// # Errors
    /// Returns `ViscaError` if the zoom command cannot be executed.
    fn zoom_in(&self, speed: ZoomSpeed) -> Result<(), ViscaError>;

    /// Start zooming out at the specified speed.
    ///
    /// # Errors
    /// Returns `ViscaError` if the zoom command cannot be executed.
    fn zoom_out(&self, speed: ZoomSpeed) -> Result<(), ViscaError>;

    /// Stop any zoom movement.
    ///
    /// # Errors
    /// Returns `ViscaError` if the zoom stop command cannot be executed.
    fn zoom_stop(&self) -> Result<(), ViscaError>;

    // Preset Management

    /// Save the current camera position to a preset (0-89).
    ///
    /// # Errors
    /// Returns `ViscaError` if the preset number is invalid or command cannot be executed.
    fn save_current_as_preset(&self, preset_number: u8) -> Result<(), ViscaError>;

    /// Recall a saved preset position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the preset number is invalid or command cannot be executed.
    fn recall_preset(&self, preset_number: u8) -> Result<(), ViscaError>;

    /// Clear/reset a preset.
    ///
    /// # Errors
    /// Returns `ViscaError` if the preset number is invalid or command cannot be executed.
    fn clear_preset(&self, preset_number: u8) -> Result<(), ViscaError>;

    // Pan/Tilt Control

    /// Get the current pan/tilt position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the position cannot be queried.
    fn pan_tilt_position(&self) -> Result<(i16, i16), ViscaError>;

    /// Move to home position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the home command cannot be executed.
    fn move_home(&self) -> Result<(), ViscaError>;

    /// Start moving in a direction at specified speeds.
    ///
    /// # Errors
    /// Returns `ViscaError` if the movement command cannot be executed.
    fn start_moving(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), ViscaError>;

    /// Stop all pan/tilt movement.
    ///
    /// # Errors
    /// Returns `ViscaError` if the stop command cannot be executed.
    fn stop_moving(&self) -> Result<(), ViscaError>;

    /// Move to an absolute pan/tilt position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the position command cannot be executed.
    fn move_to_position(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), ViscaError>;

    // Focus Control

    /// Get the current focus position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the focus position cannot be queried.
    fn focus_position(&self) -> Result<u16, ViscaError>;

    /// Set focus mode to auto.
    ///
    /// # Errors
    /// Returns `ViscaError` if the focus command cannot be executed.
    fn focus_auto(&self) -> Result<(), ViscaError>;

    /// Set focus mode to manual.
    ///
    /// # Errors
    /// Returns `ViscaError` if the focus command cannot be executed.
    fn focus_manual(&self) -> Result<(), ViscaError>;

    /// Focus to a specific position (manual mode).
    ///
    /// # Errors
    /// Returns `ViscaError` if the focus command cannot be executed.
    fn focus_to_position(&self, position: u16) -> Result<(), ViscaError>;

    /// Start focusing near at specified speed.
    ///
    /// # Errors
    /// Returns `ViscaError` if the focus command cannot be executed.
    fn focus_near(&self, speed: FocusSpeed) -> Result<(), ViscaError>;

    /// Start focusing far at specified speed.
    ///
    /// # Errors
    /// Returns `ViscaError` if the focus command cannot be executed.
    fn focus_far(&self, speed: FocusSpeed) -> Result<(), ViscaError>;

    /// Stop focus movement.
    ///
    /// # Errors
    /// Returns `ViscaError` if the focus stop command cannot be executed.
    fn focus_stop(&self) -> Result<(), ViscaError>;
}

// Implement for owned ViscaClient
#[cfg(feature = "blocking-client")]
impl CameraExt for ViscaClient {
    fn is_powered_on(&self) -> Result<bool, ViscaError> {
        let response = self.send(&InquiryCommand::Power)?;
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) => Ok(on),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    fn power_on(&self) -> Result<(), ViscaError> {
        self.send(&PowerCommand { power: Power::On })?;
        Ok(())
    }

    fn power_off(&self) -> Result<(), ViscaError> {
        self.send(&PowerCommand {
            power: Power::Standby,
        })?;
        Ok(())
    }

    fn zoom_position(&self) -> Result<u16, ViscaError> {
        let response = self.send(&InquiryCommand::ZoomPosition)?;
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) => {
                Ok(position)
            }
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    fn zoom_to_position(&self, position: u16) -> Result<(), ViscaError> {
        self.send(&ZoomCommand::Direct(position))?;
        Ok(())
    }

    fn zoom_in(&self, speed: ZoomSpeed) -> Result<(), ViscaError> {
        self.send(&ZoomCommand::TeleVariable(speed))?;
        Ok(())
    }

    fn zoom_out(&self, speed: ZoomSpeed) -> Result<(), ViscaError> {
        self.send(&ZoomCommand::WideVariable(speed))?;
        Ok(())
    }

    fn zoom_stop(&self) -> Result<(), ViscaError> {
        self.send(&ZoomCommand::Stop)?;
        Ok(())
    }

    fn save_current_as_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send(&PresetCommand {
            action: PresetAction::Set,
            preset_number: preset_num,
        })?;
        Ok(())
    }

    fn recall_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send(&PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset_num,
        })?;
        Ok(())
    }

    fn clear_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send(&PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset_num,
        })?;
        Ok(())
    }

    fn pan_tilt_position(&self) -> Result<(i16, i16), ViscaError> {
        let response = self.send(&InquiryCommand::PanTiltPosition)?;
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((pan, tilt))
            }
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    fn move_home(&self) -> Result<(), ViscaError> {
        self.send(&PanTiltCommand::Home)?;
        Ok(())
    }

    fn start_moving(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), ViscaError> {
        self.send(&PanTiltCommand::Move {
            direction,
            pan_speed,
            tilt_speed,
        })?;
        Ok(())
    }

    fn stop_moving(&self) -> Result<(), ViscaError> {
        self.send(&PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        })?;
        Ok(())
    }

    fn move_to_position(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), ViscaError> {
        self.send(&PanTiltCommand::AbsolutePosition {
            pan,
            tilt,
            pan_speed,
            tilt_speed,
        })?;
        Ok(())
    }

    fn focus_position(&self) -> Result<u16, ViscaError> {
        let response = self.send(&InquiryCommand::FocusPosition)?;
        match response {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusPosition { position }) => {
                Ok(position)
            }
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    fn focus_auto(&self) -> Result<(), ViscaError> {
        self.send(&FocusCommand::Auto)?;
        Ok(())
    }

    fn focus_manual(&self) -> Result<(), ViscaError> {
        self.send(&FocusCommand::Manual)?;
        Ok(())
    }

    fn focus_to_position(&self, position: u16) -> Result<(), ViscaError> {
        self.send(&FocusCommand::Direct(position))?;
        Ok(())
    }

    fn focus_near(&self, speed: FocusSpeed) -> Result<(), ViscaError> {
        self.send(&FocusCommand::NearVariable(speed))?;
        Ok(())
    }

    fn focus_far(&self, speed: FocusSpeed) -> Result<(), ViscaError> {
        self.send(&FocusCommand::FarVariable(speed))?;
        Ok(())
    }

    fn focus_stop(&self) -> Result<(), ViscaError> {
        self.send(&FocusCommand::Stop)?;
        Ok(())
    }
}

// Also implement for Arc<ViscaClient> for shared ownership scenarios
#[cfg(feature = "blocking-client")]
impl CameraExt for Arc<ViscaClient> {
    fn is_powered_on(&self) -> Result<bool, ViscaError> {
        (**self).is_powered_on()
    }

    fn power_on(&self) -> Result<(), ViscaError> {
        (**self).power_on()
    }

    fn power_off(&self) -> Result<(), ViscaError> {
        (**self).power_off()
    }

    fn zoom_position(&self) -> Result<u16, ViscaError> {
        (**self).zoom_position()
    }

    fn zoom_to_position(&self, position: u16) -> Result<(), ViscaError> {
        (**self).zoom_to_position(position)
    }

    fn zoom_in(&self, speed: ZoomSpeed) -> Result<(), ViscaError> {
        (**self).zoom_in(speed)
    }

    fn zoom_out(&self, speed: ZoomSpeed) -> Result<(), ViscaError> {
        (**self).zoom_out(speed)
    }

    fn zoom_stop(&self) -> Result<(), ViscaError> {
        (**self).zoom_stop()
    }

    fn save_current_as_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        (**self).save_current_as_preset(preset_number)
    }

    fn recall_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        (**self).recall_preset(preset_number)
    }

    fn clear_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        (**self).clear_preset(preset_number)
    }

    fn pan_tilt_position(&self) -> Result<(i16, i16), ViscaError> {
        (**self).pan_tilt_position()
    }

    fn move_home(&self) -> Result<(), ViscaError> {
        (**self).move_home()
    }

    fn start_moving(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), ViscaError> {
        (**self).start_moving(direction, pan_speed, tilt_speed)
    }

    fn stop_moving(&self) -> Result<(), ViscaError> {
        (**self).stop_moving()
    }

    fn move_to_position(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), ViscaError> {
        (**self).move_to_position(pan, tilt, pan_speed, tilt_speed)
    }

    fn focus_position(&self) -> Result<u16, ViscaError> {
        (**self).focus_position()
    }

    fn focus_auto(&self) -> Result<(), ViscaError> {
        (**self).focus_auto()
    }

    fn focus_manual(&self) -> Result<(), ViscaError> {
        (**self).focus_manual()
    }

    fn focus_to_position(&self, position: u16) -> Result<(), ViscaError> {
        (**self).focus_to_position(position)
    }

    fn focus_near(&self, speed: FocusSpeed) -> Result<(), ViscaError> {
        (**self).focus_near(speed)
    }

    fn focus_far(&self, speed: FocusSpeed) -> Result<(), ViscaError> {
        (**self).focus_far(speed)
    }

    fn focus_stop(&self) -> Result<(), ViscaError> {
        (**self).focus_stop()
    }
}

/// Async-specific camera control extension.
///
/// This trait provides async versions of time-consuming operations
/// that benefit from async execution.
#[cfg(feature = "async-client")]
#[allow(async_fn_in_trait)]
pub trait AsyncCameraExt {
    /// Power cycle the camera with a delay between off and on.
    ///
    /// # Errors
    /// Returns `ViscaError` if either power command fails.
    async fn power_cycle(&self, delay: Duration) -> Result<(), ViscaError>;

    /// Wait for the camera to power on, with timeout.
    ///
    /// # Errors
    /// Returns `ViscaError` if the camera doesn't power on within the timeout.
    async fn wait_for_power_on(
        &self,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<(), ViscaError>;

    /// Smoothly zoom to a position and wait for completion.
    ///
    /// # Errors
    /// Returns `ViscaError` if the zoom operation fails or times out.
    async fn zoom_to_and_wait(&self, position: u16, timeout: Duration) -> Result<(), ViscaError>;

    /// Move to a position and wait for completion.
    ///
    /// # Errors
    /// Returns `ViscaError` if the move operation fails or times out.
    async fn move_to_and_wait(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        timeout: Duration,
    ) -> Result<(), ViscaError>;

    /// Patrol between multiple preset positions.
    ///
    /// # Errors
    /// Returns `ViscaError` if any preset recall fails.
    async fn patrol_presets(&self, presets: &[u8], dwell_time: Duration) -> Result<(), ViscaError>;
}

#[cfg(feature = "async-client")]
impl AsyncCameraExt for ViscaClient {
    async fn power_cycle(&self, delay: Duration) -> Result<(), ViscaError> {
        self.send_async(&PowerCommand {
            power: Power::Standby,
        })
        .await?;
        tokio::time::sleep(delay).await;
        self.send_async(&PowerCommand { power: Power::On }).await?;
        Ok(())
    }

    async fn wait_for_power_on(
        &self,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<(), ViscaError> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(poll_interval);

        loop {
            let response = self.send_async(&InquiryCommand::Power).await?;
            if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) = response {
                if on {
                    return Ok(());
                }
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(ViscaError::Timeout);
            }

            interval.tick().await;
        }
    }

    async fn zoom_to_and_wait(&self, target: u16, timeout: Duration) -> Result<(), ViscaError> {
        self.send_async(&ZoomCommand::Direct(target)).await?;

        let deadline = tokio::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            let response = self.send_async(&InquiryCommand::ZoomPosition).await?;
            if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) =
                response
            {
                if position == target {
                    return Ok(());
                }
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(ViscaError::Timeout);
            }

            interval.tick().await;
        }
    }

    async fn move_to_and_wait(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        timeout: Duration,
    ) -> Result<(), ViscaError> {
        self.send_async(&PanTiltCommand::AbsolutePosition {
            pan,
            tilt,
            pan_speed,
            tilt_speed,
        })
        .await?;

        let deadline = tokio::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            let response = self.send_async(&InquiryCommand::PanTiltPosition).await?;
            if let ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition {
                pan: current_pan,
                tilt: current_tilt,
            }) = response
            {
                if current_pan == pan && current_tilt == tilt {
                    return Ok(());
                }
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(ViscaError::Timeout);
            }

            interval.tick().await;
        }
    }

    async fn patrol_presets(&self, presets: &[u8], dwell_time: Duration) -> Result<(), ViscaError> {
        for &preset in presets.iter().cycle() {
            self.send_async(&PresetCommand {
                preset_number: PresetNumber::new(preset)?,
                action: PresetAction::Recall,
            })
            .await?;
            tokio::time::sleep(dwell_time).await;
        }
        Ok(())
    }
}

#[cfg(feature = "async-client")]
impl AsyncCameraExt for Arc<ViscaClient> {
    async fn power_cycle(&self, delay: Duration) -> Result<(), ViscaError> {
        (**self).power_cycle(delay).await
    }

    async fn wait_for_power_on(
        &self,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<(), ViscaError> {
        (**self).wait_for_power_on(timeout, poll_interval).await
    }

    async fn zoom_to_and_wait(&self, position: u16, timeout: Duration) -> Result<(), ViscaError> {
        (**self).zoom_to_and_wait(position, timeout).await
    }

    async fn move_to_and_wait(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        timeout: Duration,
    ) -> Result<(), ViscaError> {
        (**self)
            .move_to_and_wait(pan, tilt, pan_speed, tilt_speed, timeout)
            .await
    }

    async fn patrol_presets(&self, presets: &[u8], dwell_time: Duration) -> Result<(), ViscaError> {
        (**self).patrol_presets(presets, dwell_time).await
    }
}
