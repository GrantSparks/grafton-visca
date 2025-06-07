//! Async implementations of control extension traits for `AsyncViscaClient`.

// Crate imports
use crate::{
    async_client::AsyncViscaClient,
    command::{
        focus::FocusCommand,
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        preset::{PresetAction, PresetCommand, PresetNumber},
        zoom::{ZoomCommand, ZoomSpeed},
    },
    ViscaError,
};

impl AsyncViscaClient {
    // Pan/Tilt Control Methods

    /// Move camera to an absolute pan/tilt position.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if speeds are out of valid range,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn move_to_position(
        &self,
        pan: i16,
        tilt: i16,
        speed: Option<(u8, u8)>,
    ) -> Result<(), ViscaError> {
        let (pan_speed, tilt_speed) = speed.unwrap_or((18, 14));
        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
            pan,
            tilt,
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Move camera relative to its current position.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if speeds are out of valid range,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn move_relative(
        &self,
        pan_delta: i16,
        tilt_delta: i16,
        speed: Option<(u8, u8)>,
    ) -> Result<(), ViscaError> {
        let (pan_speed, tilt_speed) = speed.unwrap_or((18, 14));
        let command = PanTiltCommand::RelativePosition {
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
            pan: pan_delta,
            tilt: tilt_delta,
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Start continuous pan/tilt movement in the specified direction.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if speeds are out of valid range,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn start_moving(
        &self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), ViscaError> {
        let pan_speed = PanSpeed::new(pan_speed)
            .map_err(|_| ViscaError::InvalidParameter("Pan speed must be 0-24".into()))?;
        let tilt_speed = TiltSpeed::new(tilt_speed)
            .map_err(|_| ViscaError::InvalidParameter("Tilt speed must be 0-20".into()))?;
        let command = PanTiltCommand::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Stop all pan/tilt movement.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn stop_movement(&self) -> Result<(), ViscaError> {
        let command = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::ZERO,
            tilt_speed: TiltSpeed::ZERO,
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Return camera to home position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn go_home(&self) -> Result<(), ViscaError> {
        let command = PanTiltCommand::Home;
        self.send(&command).await?;
        Ok(())
    }

    // Zoom Control Methods

    /// Move zoom to an absolute position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn zoom_to(&self, position: u16) -> Result<(), ViscaError> {
        let command = ZoomCommand::Direct(position);
        self.send(&command).await?;
        Ok(())
    }

    /// Start zooming in (telephoto direction).
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if speed is greater than 7,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn zoom_in(&self, speed: Option<u8>) -> Result<(), ViscaError> {
        let command = if let Some(s) = speed {
            if s > 7 {
                return Err(ViscaError::InvalidParameter(
                    "Zoom speed must be 0-7".into(),
                ));
            }
            ZoomCommand::TeleVariable(ZoomSpeed::new(s)?)
        } else {
            ZoomCommand::TeleStandard
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Start zooming out (wide direction).
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if speed is greater than 7,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn zoom_out(&self, speed: Option<u8>) -> Result<(), ViscaError> {
        let command = if let Some(s) = speed {
            if s > 7 {
                return Err(ViscaError::InvalidParameter(
                    "Zoom speed must be 0-7".into(),
                ));
            }
            ZoomCommand::WideVariable(ZoomSpeed::new(s)?)
        } else {
            ZoomCommand::WideStandard
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Stop zoom movement.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn stop_zoom(&self) -> Result<(), ViscaError> {
        let command = ZoomCommand::Stop;
        self.send(&command).await?;
        Ok(())
    }

    // Focus Control Methods

    /// Enable or disable auto-focus mode.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn set_auto_focus(&self, enabled: bool) -> Result<(), ViscaError> {
        let command = if enabled {
            FocusCommand::Auto
        } else {
            FocusCommand::Manual
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Move focus to an absolute position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn focus_to(&self, position: u16) -> Result<(), ViscaError> {
        let command = FocusCommand::Direct(position);
        self.send(&command).await?;
        Ok(())
    }

    /// Start focusing near (closer to camera).
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if speed is greater than 7,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn focus_near(&self, speed: Option<u8>) -> Result<(), ViscaError> {
        let command = if let Some(s) = speed {
            if s > 7 {
                return Err(ViscaError::InvalidParameter(
                    "Focus speed must be 0-7".into(),
                ));
            }
            FocusCommand::NearVariable(s)
        } else {
            FocusCommand::NearStandard
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Start focusing far (farther from camera).
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if speed is greater than 7,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn focus_far(&self, speed: Option<u8>) -> Result<(), ViscaError> {
        let command = if let Some(s) = speed {
            if s > 7 {
                return Err(ViscaError::InvalidParameter(
                    "Focus speed must be 0-7".into(),
                ));
            }
            FocusCommand::FarVariable(s)
        } else {
            FocusCommand::FarStandard
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Stop focus movement.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn stop_focus(&self) -> Result<(), ViscaError> {
        let command = FocusCommand::Stop;
        self.send(&command).await?;
        Ok(())
    }

    /// Trigger one-push auto-focus.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn trigger_one_push_focus(&self) -> Result<(), ViscaError> {
        let command = FocusCommand::OnePushTrigger;
        self.send(&command).await?;
        Ok(())
    }

    // Preset Management Methods

    /// Save the current camera position to a preset.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if preset_number is invalid,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn save_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number: PresetNumber::new(preset_number)?,
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Recall a saved preset position.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if preset_number is invalid,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn recall_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number: PresetNumber::new(preset_number)?,
        };
        self.send(&command).await?;
        Ok(())
    }

    /// Reset a preset to its default state.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if preset_number is invalid,
    /// or `ViscaError` if the command fails to send or the camera returns an error.
    pub async fn reset_preset(&self, preset_number: u8) -> Result<(), ViscaError> {
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number: PresetNumber::new(preset_number)?,
        };
        self.send(&command).await?;
        Ok(())
    }
}
