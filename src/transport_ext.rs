//! Extension trait providing convenience methods for common VISCA operations.

use crate::{
    command::{
        exposure::{
            ExposureCommand, ExposureCompensationCommand, ExposureCompensationLevel, ExposureMode,
            IrisCommand,
        },
        focus::FocusCommand,
        gain::GainCommand,
        image::BacklightCommand,
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        power::{Power, PowerCommand},
        preset::{PresetAction, PresetCommand, PresetNumber},
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
        zoom::ZoomCommand,
    },
    ViscaDevice, ViscaError, ViscaResponse,
};

/// Extension trait providing convenience methods for common VISCA operations.
///
/// This trait is automatically implemented for all types that implement `ViscaDevice`,
/// providing a more ergonomic API for common camera control operations.
///
/// # Example
/// ```no_run
/// # #[cfg(feature = "blocking-client")]
/// # {
/// # use grafton_visca::{ViscaClient, ViscaTransportExt, ViscaError};
/// # use grafton_visca::command::exposure::ExposureMode;
/// let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
///
/// // Simple one-line operations
/// client.power_on()?;
/// client.home()?;
/// client.set_exposure_mode(ExposureMode::Auto)?;
/// # }
/// # Ok::<(), ViscaError>(())
/// ```
pub trait ViscaTransportExt: ViscaDevice {
    /// Powers on the camera.
    fn power_on(&mut self) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&PowerCommand { power: Power::On })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Powers off the camera (standby mode).
    fn power_off(&mut self) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&PowerCommand {
            power: Power::Standby,
        })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Moves the camera to the home position.
    fn home(&mut self) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&PanTiltCommand::Home)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Recalls a preset position.
    ///
    /// # Arguments
    /// * `preset_id` - The preset number to recall (typically 0-89)
    fn recall_preset(&mut self, preset_id: u8) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&PresetCommand {
            preset_number: PresetNumber::new(preset_id)?,
            action: PresetAction::Recall,
        })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Saves the current position as a preset.
    ///
    /// # Arguments
    /// * `preset_id` - The preset number to save (typically 0-89)
    fn save_preset(&mut self, preset_id: u8) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&PresetCommand {
            preset_number: PresetNumber::new(preset_id)?,
            action: PresetAction::Set,
        })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Sets the exposure mode.
    fn set_exposure_mode(&mut self, mode: ExposureMode) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&ExposureCommand { mode })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Sets the white balance mode.
    fn set_white_balance(&mut self, mode: WhiteBalanceMode) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&WhiteBalanceCommand { mode })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Starts moving the camera in the specified direction.
    ///
    /// # Arguments
    /// * `direction` - The direction to move
    /// * `pan_speed` - Pan speed (0x01-0x18)
    /// * `tilt_speed` - Tilt speed (0x01-0x14)
    fn move_start(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
        })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Stops camera movement.
    fn move_stop(&mut self) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Moves the camera to an absolute position.
    ///
    /// # Arguments
    /// * `pan` - Pan position in VISCA units
    /// * `tilt` - Tilt position in VISCA units
    /// * `pan_speed` - Pan speed (0x01-0x18)
    /// * `tilt_speed` - Tilt speed (0x01-0x14)
    fn move_absolute(
        &mut self,
        pan: i16,
        tilt: i16,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&PanTiltCommand::AbsolutePosition {
            pan,
            tilt,
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
        })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Starts zooming in (tele direction).
    fn zoom_in(&mut self) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&ZoomCommand::TeleStandard)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Starts zooming out (wide direction).
    fn zoom_out(&mut self) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&ZoomCommand::WideStandard)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Stops zooming.
    fn zoom_stop(&mut self) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&ZoomCommand::Stop)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Sets zoom to an absolute position.
    ///
    /// # Arguments
    /// * `position` - Zoom position in VISCA units (0x0000-0x4000 for most cameras)
    fn zoom_direct(&mut self, position: u16) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&ZoomCommand::Direct(position))? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Sets focus to auto mode.
    fn set_focus_auto(&mut self) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&FocusCommand::Auto)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Sets focus to manual mode.
    fn set_focus_manual(&mut self) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&FocusCommand::Manual)? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Enables or disables backlight compensation.
    fn set_backlight(&mut self, enabled: bool) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&BacklightCommand { status: enabled })? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Sets exposure compensation value.
    ///
    /// # Arguments
    /// * `value` - Exposure compensation value (-7 to +7)
    fn set_exposure_compensation(&mut self, value: i8) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&ExposureCompensationCommand::Direct(
            ExposureCompensationLevel::new(value)?,
        ))? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Sets iris value directly.
    ///
    /// # Arguments
    /// * `value` - Iris value in VISCA units
    fn set_iris(&mut self, value: u8) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&IrisCommand::Direct(value))? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Sets gain value directly.
    ///
    /// # Arguments
    /// * `value` - Gain value in VISCA units
    fn set_gain(&mut self, value: u16) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&GainCommand::Direct(value))? {
            ViscaResponse::Completion => Ok(()),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }
}

// Blanket implementation for all types that implement ViscaDevice
impl<T: ViscaDevice> ViscaTransportExt for T {}
