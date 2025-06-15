//! Type-safe command methods for Camera<P>.

use crate::{
    command::{
        exposure::{ExposureCommand, ExposureMode},
        focus::FocusCommand,
        gain::GainCommand,
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        power::{Power, PowerCommand},
        preset::{PresetAction, PresetCommand, PresetNumber},
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
        zoom::ZoomCommand,
    },
    error::Error,
    types::GainValue,
};

use super::{
    units::{Degrees, Normalized, ViscaUnits},
    Camera, CameraProfile,
};

impl<P: CameraProfile> Camera<P> {
    /// Power on the camera.
    pub async fn power_on(&mut self) -> Result<(), Error> {
        let command = PowerCommand { power: Power::On };
        self.transport.send_command(&command).await
    }

    /// Power off the camera.
    pub async fn power_off(&mut self) -> Result<(), Error> {
        let command = PowerCommand {
            power: Power::Standby,
        };
        self.transport.send_command(&command).await
    }

    /// Set the camera to an absolute pan/tilt position in degrees.
    pub async fn set_position(
        &mut self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
    ) -> Result<(), Error> {
        // Convert degrees to VISCA units using the camera profile
        let pan_units = self.profile.pan_degrees_to_units(pan.0);
        let tilt_units = self.profile.tilt_degrees_to_units(tilt.0);

        // Validate ranges
        if !P::PAN_RANGE.contains(&pan_units) {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan".to_string(),
                value: pan_units as i32,
                min: *P::PAN_RANGE.start() as i32,
                max: *P::PAN_RANGE.end() as i32,
            });
        }

        if !P::TILT_RANGE.contains(&tilt_units) {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt".to_string(),
                value: tilt_units as i32,
                min: *P::TILT_RANGE.start() as i32,
                max: *P::TILT_RANGE.end() as i32,
            });
        }

        // Create and send the absolute position command
        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?, // Use half speed for smooth movement
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: pan_units,
            tilt: tilt_units,
        };

        self.transport.send_command(&command).await
    }

    /// Set the camera to an absolute position using VISCA units.
    pub async fn set_position_units(
        &mut self,
        pan: ViscaUnits<i16>,
        tilt: ViscaUnits<i16>,
    ) -> Result<(), Error> {
        // Validate ranges
        if !P::PAN_RANGE.contains(&pan.0) {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan".to_string(),
                value: pan.0 as i32,
                min: *P::PAN_RANGE.start() as i32,
                max: *P::PAN_RANGE.end() as i32,
            });
        }

        if !P::TILT_RANGE.contains(&tilt.0) {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt".to_string(),
                value: tilt.0 as i32,
                min: *P::TILT_RANGE.start() as i32,
                max: *P::TILT_RANGE.end() as i32,
            });
        }

        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?,
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: pan.0,
            tilt: tilt.0,
        };

        self.transport.send_command(&command).await
    }

    /// Set the camera position using normalized coordinates (-1.0 to 1.0).
    pub async fn set_position_normalized(
        &mut self,
        pan: Normalized<f32>,
        tilt: Normalized<f32>,
    ) -> Result<(), Error> {
        // Clamp normalized values to -1.0 to 1.0
        let pan_norm = pan.0.clamp(-1.0, 1.0);
        let tilt_norm = tilt.0.clamp(-1.0, 1.0);

        // Convert normalized to VISCA units
        let pan_range = P::PAN_RANGE.end() - P::PAN_RANGE.start();
        let pan_units = (pan_norm * pan_range as f32 / 2.0) as i16;

        let tilt_range = P::TILT_RANGE.end() - P::TILT_RANGE.start();
        let tilt_units = (tilt_norm * tilt_range as f32 / 2.0) as i16;

        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?,
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: pan_units,
            tilt: tilt_units,
        };

        self.transport.send_command(&command).await
    }

    /// Move the camera continuously in a direction.
    pub async fn move_continuous(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error> {
        // Ensure speeds are within valid range - 0 is always valid
        let safe_pan_speed = pan_speed.min(P::MAX_PAN_SPEED);
        let safe_tilt_speed = tilt_speed.min(P::MAX_TILT_SPEED);

        let command = PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::new(safe_pan_speed).map_err(|_| {
                Error::InvalidParameter(format!("Invalid pan speed: {}", safe_pan_speed))
            })?,
            tilt_speed: TiltSpeed::new(safe_tilt_speed).map_err(|_| {
                Error::InvalidParameter(format!("Invalid tilt speed: {}", safe_tilt_speed))
            })?,
        };

        self.transport.send_command(&command).await
    }

    /// Stop all camera movement.
    pub async fn stop(&mut self) -> Result<(), Error> {
        let command = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)
                .map_err(|_| Error::InvalidParameter("Invalid pan speed: 0".to_string()))?,
            tilt_speed: TiltSpeed::new(0)
                .map_err(|_| Error::InvalidParameter("Invalid tilt speed: 0".to_string()))?,
        };
        self.transport.send_command(&command).await
    }

    /// Move camera to home position.
    pub async fn home(&mut self) -> Result<(), Error> {
        self.transport.send_command(&PanTiltCommand::Home).await
    }

    /// Recall a preset position.
    pub async fn recall_preset(&mut self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number,
        };
        self.transport.send_command(&command).await
    }

    /// Set a preset position.
    pub async fn set_preset(&mut self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number,
        };
        self.transport.send_command(&command).await
    }

    /// Clear a preset position.
    pub async fn clear_preset(&mut self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number,
        };
        self.transport.send_command(&command).await
    }

    /// Zoom in at standard speed.
    pub async fn zoom_in(&mut self) -> Result<(), Error> {
        self.transport
            .send_command(&ZoomCommand::ZoomInStandard)
            .await
    }

    /// Zoom out at standard speed.
    pub async fn zoom_out(&mut self) -> Result<(), Error> {
        self.transport
            .send_command(&ZoomCommand::ZoomOutStandard)
            .await
    }

    /// Stop zooming.
    pub async fn zoom_stop(&mut self) -> Result<(), Error> {
        self.transport.send_command(&ZoomCommand::Stop).await
    }

    /// Set zoom to direct position.
    pub async fn set_zoom(&mut self, position: u16) -> Result<(), Error> {
        if !P::ZOOM_RANGE.contains(&position) {
            return Err(Error::ParameterOutOfRange {
                parameter: "zoom".to_string(),
                value: position as i32,
                min: *P::ZOOM_RANGE.start() as i32,
                max: *P::ZOOM_RANGE.end() as i32,
            });
        }

        self.transport
            .send_command(&ZoomCommand::Direct(position))
            .await
    }

    /// Set focus mode to auto.
    pub async fn focus_auto(&mut self) -> Result<(), Error> {
        self.transport.send_command(&FocusCommand::Auto).await
    }

    /// Set focus mode to manual.
    pub async fn focus_manual(&mut self) -> Result<(), Error> {
        self.transport.send_command(&FocusCommand::Manual).await
    }

    /// Set focus to direct position (manual mode).
    pub async fn set_focus(&mut self, position: u16) -> Result<(), Error> {
        if !P::FOCUS_RANGE.contains(&position) {
            return Err(Error::ParameterOutOfRange {
                parameter: "focus".to_string(),
                value: position as i32,
                min: *P::FOCUS_RANGE.start() as i32,
                max: *P::FOCUS_RANGE.end() as i32,
            });
        }

        self.transport
            .send_command(&FocusCommand::Direct(position))
            .await
    }

    /// Set exposure mode.
    pub async fn set_exposure_mode(&mut self, mode: ExposureMode) -> Result<(), Error> {
        let command = ExposureCommand { mode };
        self.transport.send_command(&command).await
    }

    /// Set white balance mode.
    pub async fn set_white_balance_mode(&mut self, mode: WhiteBalanceMode) -> Result<(), Error> {
        let command = WhiteBalanceCommand { mode };
        self.transport.send_command(&command).await
    }

    /// Set gain value.
    pub async fn set_gain(&mut self, gain: P::GainValue) -> Result<(), Error> {
        let value: u8 = gain.into();
        let gain_value = GainValue::new(value)?;
        self.transport
            .send_command(&GainCommand::Direct(gain_value))
            .await
    }
}
