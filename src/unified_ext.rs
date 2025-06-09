//! Unified extension traits for VISCA camera control.
//!
//! This module provides a clean, unified API that works seamlessly with both
//! async and blocking contexts. The key design principle is to leverage the
//! existing `Client` which already handles async/sync unification internally.
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
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "blocking-client")]
//! # {
//! use grafton_visca::{Client, Error, CameraExt};
//! let camera = Client::connect_udp("192.168.1.100:5678")?;
//! // Works in both sync and async contexts!
//! if camera.is_powered_on()? {
//!     camera.zoom_to_position(0x4000)?;
//!     camera.save_current_as_preset(1)?;
//! }
//! # }
//! # Ok(())
//! # }
//! ```

use crate::{
    command::{
        focus::FocusSpeed,
        pan_tilt::{PanSpeed, PanTiltDirection, TiltSpeed},
    },
    error::Error,
    unified_client::Client,
};

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::{
    command::{
        pan_tilt::PanTiltCommand,
        power::{Power, PowerCommand},
        preset::{PresetAction, PresetCommand, PresetNumber},
        zoom::ZoomCommand,
        InquiryResponse,
        inquiry::InquiryCommand,
    },
    Response,
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
    /// Returns `Error` if the power status cannot be queried.
    fn is_powered_on(&self) -> Result<bool, Error>;

    // Zoom Control

    /// Get the current zoom position.
    ///
    /// # Errors
    /// Returns `Error` if the zoom position cannot be queried.
    fn zoom_position(&self) -> Result<u16, Error>;

    /// Zoom to a specific position (0x0000 to 0xFFFF).
    ///
    /// # Errors
    /// Returns `Error` if the zoom command cannot be executed.
    fn zoom_to_position(&self, position: u16) -> Result<(), Error>;

    // Preset Management

    /// Set (save) the current camera position to a preset (0-89).
    ///
    /// # Errors
    /// Returns `Error` if the preset number is invalid or command cannot be executed.
    fn set_preset(&self, preset_number: u8) -> Result<(), Error>;

    /// Recall a saved preset position.
    ///
    /// # Errors
    /// Returns `Error` if the preset number is invalid or command cannot be executed.
    fn recall_preset(&self, preset_number: u8) -> Result<(), Error>;

    /// Clear/reset a preset.
    ///
    /// # Errors
    /// Returns `Error` if the preset number is invalid or command cannot be executed.
    fn clear_preset(&self, preset_number: u8) -> Result<(), Error>;

    // Pan/Tilt Control

    /// Get the current pan/tilt position.
    ///
    /// # Errors
    /// Returns `Error` if the position cannot be queried.
    fn pan_tilt_position(&self) -> Result<(i16, i16), Error>;

    /// Move to home position.
    ///
    /// # Errors
    /// Returns `Error` if the home command cannot be executed.
    fn move_home(&self) -> Result<(), Error>;

    /// Start moving in a direction at specified speeds.
    ///
    /// # Errors
    /// Returns `Error` if the movement command cannot be executed.
    fn start_moving(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>;

    /// Stop all pan/tilt movement.
    ///
    /// # Errors
    /// Returns `Error` if the stop command cannot be executed.
    fn stop_moving(&self) -> Result<(), Error>;

    /// Move to an absolute pan/tilt position.
    ///
    /// # Errors
    /// Returns `Error` if the position command cannot be executed.
    fn move_to_position(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>;

    // Focus Control

    /// Get the current focus position.
    ///
    /// # Errors
    /// Returns `Error` if the focus position cannot be queried.
    fn focus_position(&self) -> Result<u16, Error>;

    /// Set focus mode to auto.
    ///
    /// # Errors
    /// Returns `Error` if the focus command cannot be executed.
    fn focus_auto(&self) -> Result<(), Error>;

    /// Set focus mode to manual.
    ///
    /// # Errors
    /// Returns `Error` if the focus command cannot be executed.
    fn focus_manual(&self) -> Result<(), Error>;

    /// Focus to a specific position (manual mode).
    ///
    /// # Errors
    /// Returns `Error` if the focus command cannot be executed.
    fn focus_to_position(&self, position: u16) -> Result<(), Error>;

    /// Start focusing near at specified speed.
    ///
    /// # Errors
    /// Returns `Error` if the focus command cannot be executed.
    fn focus_near(&self, speed: FocusSpeed) -> Result<(), Error>;

    /// Start focusing far at specified speed.
    ///
    /// # Errors
    /// Returns `Error` if the focus command cannot be executed.
    fn focus_far(&self, speed: FocusSpeed) -> Result<(), Error>;

    /// Stop focus movement.
    ///
    /// # Errors
    /// Returns `Error` if the focus stop command cannot be executed.
    fn focus_stop(&self) -> Result<(), Error>;
}

// Implement for owned Client
#[cfg(feature = "blocking-client")]
impl CameraExt for Client {
    fn is_powered_on(&self) -> Result<bool, Error> {
        let response = self.send(&InquiryCommand::Power)?;
        match response {
            Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn zoom_position(&self) -> Result<u16, Error> {
        let response = self.send(&InquiryCommand::ZoomPosition)?;
        match response {
            Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => {
                Ok(position)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn zoom_to_position(&self, position: u16) -> Result<(), Error> {
        self.send(&ZoomCommand::Direct(position))?;
        Ok(())
    }

    fn set_preset(&self, preset_number: u8) -> Result<(), Error> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send(&PresetCommand {
            action: PresetAction::Set,
            preset_number: preset_num,
        })?;
        Ok(())
    }

    fn recall_preset(&self, preset_number: u8) -> Result<(), Error> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send(&PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset_num,
        })?;
        Ok(())
    }

    fn clear_preset(&self, preset_number: u8) -> Result<(), Error> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.send(&PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset_num,
        })?;
        Ok(())
    }

    fn pan_tilt_position(&self) -> Result<(i16, i16), Error> {
        let response = self.send(&InquiryCommand::PanTiltPosition)?;
        match response {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((pan, tilt))
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn move_home(&self) -> Result<(), Error> {
        self.send(&PanTiltCommand::Home)?;
        Ok(())
    }

    fn start_moving(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error> {
        self.send(&PanTiltCommand::Move {
            direction,
            pan_speed,
            tilt_speed,
        })?;
        Ok(())
    }

    fn stop_moving(&self) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
        self.send(&PanTiltCommand::AbsolutePosition {
            pan,
            tilt,
            pan_speed,
            tilt_speed,
        })?;
        Ok(())
    }

    fn focus_position(&self) -> Result<u16, Error> {
        let response = self.send(&InquiryCommand::FocusPosition)?;
        match response {
            Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => {
                Ok(position)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn focus_auto(&self) -> Result<(), Error> {
        self.send(&FocusCommand::Auto)?;
        Ok(())
    }

    fn focus_manual(&self) -> Result<(), Error> {
        self.send(&FocusCommand::Manual)?;
        Ok(())
    }

    fn focus_to_position(&self, position: u16) -> Result<(), Error> {
        self.send(&FocusCommand::Direct(position))?;
        Ok(())
    }

    fn focus_near(&self, speed: FocusSpeed) -> Result<(), Error> {
        self.send(&FocusCommand::NearVariable(speed))?;
        Ok(())
    }

    fn focus_far(&self, speed: FocusSpeed) -> Result<(), Error> {
        self.send(&FocusCommand::FarVariable(speed))?;
        Ok(())
    }

    fn focus_stop(&self) -> Result<(), Error> {
        self.send(&FocusCommand::Stop)?;
        Ok(())
    }
}

// Also implement for Arc<Client> for shared ownership scenarios
#[cfg(feature = "blocking-client")]
impl CameraExt for Arc<Client> {
    fn is_powered_on(&self) -> Result<bool, Error> {
        (**self).is_powered_on()
    }

    fn zoom_position(&self) -> Result<u16, Error> {
        (**self).zoom_position()
    }

    fn zoom_to_position(&self, position: u16) -> Result<(), Error> {
        (**self).zoom_to_position(position)
    }

    fn set_preset(&self, preset_number: u8) -> Result<(), Error> {
        (**self).set_preset(preset_number)
    }

    fn recall_preset(&self, preset_number: u8) -> Result<(), Error> {
        (**self).recall_preset(preset_number)
    }

    fn clear_preset(&self, preset_number: u8) -> Result<(), Error> {
        (**self).clear_preset(preset_number)
    }

    fn pan_tilt_position(&self) -> Result<(i16, i16), Error> {
        (**self).pan_tilt_position()
    }

    fn move_home(&self) -> Result<(), Error> {
        (**self).move_home()
    }

    fn start_moving(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error> {
        (**self).start_moving(direction, pan_speed, tilt_speed)
    }

    fn stop_moving(&self) -> Result<(), Error> {
        (**self).stop_moving()
    }

    fn move_to_position(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error> {
        (**self).move_to_position(pan, tilt, pan_speed, tilt_speed)
    }

    fn focus_position(&self) -> Result<u16, Error> {
        (**self).focus_position()
    }

    fn focus_auto(&self) -> Result<(), Error> {
        (**self).focus_auto()
    }

    fn focus_manual(&self) -> Result<(), Error> {
        (**self).focus_manual()
    }

    fn focus_to_position(&self, position: u16) -> Result<(), Error> {
        (**self).focus_to_position(position)
    }

    fn focus_near(&self, speed: FocusSpeed) -> Result<(), Error> {
        (**self).focus_near(speed)
    }

    fn focus_far(&self, speed: FocusSpeed) -> Result<(), Error> {
        (**self).focus_far(speed)
    }

    fn focus_stop(&self) -> Result<(), Error> {
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
    /// Returns `Error` if either power command fails.
    async fn power_cycle(&self, delay: Duration) -> Result<(), Error>;

    /// Wait for the camera to power on, with timeout.
    ///
    /// # Errors
    /// Returns `Error` if the camera doesn't power on within the timeout.
    async fn wait_for_power_on(
        &self,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<(), Error>;

    /// Smoothly zoom to a position and wait for completion.
    ///
    /// # Errors
    /// Returns `Error` if the zoom operation fails or times out.
    async fn zoom_to_and_wait(&self, position: u16, timeout: Duration) -> Result<(), Error>;

    /// Move to a position and wait for completion.
    ///
    /// # Errors
    /// Returns `Error` if the move operation fails or times out.
    async fn move_to_and_wait(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        timeout: Duration,
    ) -> Result<(), Error>;

    /// Patrol between multiple preset positions.
    ///
    /// # Errors
    /// Returns `Error` if any preset recall fails.
    async fn patrol_presets(&self, presets: &[u8], dwell_time: Duration) -> Result<(), Error>;
}

#[cfg(feature = "async-client")]
impl AsyncCameraExt for Client {
    async fn power_cycle(&self, delay: Duration) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(poll_interval);

        loop {
            let response = self.send_async(&InquiryCommand::Power).await?;
            if let Response::InquiryResponse(InquiryResponse::Power { on }) = response {
                if on {
                    return Ok(());
                }
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout);
            }

            interval.tick().await;
        }
    }

    async fn zoom_to_and_wait(&self, target: u16, timeout: Duration) -> Result<(), Error> {
        self.send_async(&ZoomCommand::Direct(target)).await?;

        let deadline = tokio::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            let response = self.send_async(&InquiryCommand::ZoomPosition).await?;
            if let Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) =
                response
            {
                if position == target {
                    return Ok(());
                }
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout);
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
    ) -> Result<(), Error> {
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
            if let Response::InquiryResponse(InquiryResponse::PanTiltPosition {
                pan: current_pan,
                tilt: current_tilt,
            }) = response
            {
                if current_pan == pan && current_tilt == tilt {
                    return Ok(());
                }
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout);
            }

            interval.tick().await;
        }
    }

    async fn patrol_presets(&self, presets: &[u8], dwell_time: Duration) -> Result<(), Error> {
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
impl AsyncCameraExt for Arc<Client> {
    async fn power_cycle(&self, delay: Duration) -> Result<(), Error> {
        (**self).power_cycle(delay).await
    }

    async fn wait_for_power_on(
        &self,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<(), Error> {
        (**self).wait_for_power_on(timeout, poll_interval).await
    }

    async fn zoom_to_and_wait(&self, position: u16, timeout: Duration) -> Result<(), Error> {
        (**self).zoom_to_and_wait(position, timeout).await
    }

    async fn move_to_and_wait(
        &self,
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        timeout: Duration,
    ) -> Result<(), Error> {
        (**self)
            .move_to_and_wait(pan, tilt, pan_speed, tilt_speed, timeout)
            .await
    }

    async fn patrol_presets(&self, presets: &[u8], dwell_time: Duration) -> Result<(), Error> {
        (**self).patrol_presets(presets, dwell_time).await
    }
}
