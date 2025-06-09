//! Extension trait providing convenience methods for common VISCA operations.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
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
        response::Response,
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
    },
    error::Error,
    Transport,
};

/// Extension trait providing convenience methods for common VISCA operations.
///
/// This trait is automatically implemented for all types that implement `Transport`,
/// providing a more ergonomic API for common camera control operations.
///
/// # Example
/// ```no_run
/// # #[cfg(feature = "blocking-client")]
/// # fn example() -> Result<(), grafton_visca::Error> {
/// # use grafton_visca::{Client, TransportExt, PowerExt, Error};
/// # use grafton_visca::command::exposure::ExposureMode;
/// let mut client = Client::connect_udp("192.168.1.100:5678")?;
///
/// // Simple one-line operations
/// client.power_on()?;
/// client.home()?;
/// client.set_exposure_mode(ExposureMode::Auto)?;
/// # Ok(())
/// # }
/// ```
pub trait TransportExt: Transport {
    /// Moves the camera to the home position.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    fn home(&mut self) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&PanTiltCommand::Home)? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Sets the exposure mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    fn set_exposure_mode(&mut self, mode: ExposureMode) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&ExposureCommand { mode })? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Sets the white balance mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    fn set_white_balance(&mut self, mode: WhiteBalanceMode) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&WhiteBalanceCommand { mode })? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Starts moving the camera in the specified direction.
    ///
    /// # Arguments
    /// * `direction` - The direction to move
    /// * `pan_speed` - Pan speed (0x01-0x18)
    /// * `tilt_speed` - Tilt speed (0x01-0x14)
    ///
    /// # Errors
    /// Returns `Error::InvalidParameter` if speeds are out of valid range,
    /// or `Error` if the command fails to send or the camera returns an error.
    fn move_start(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
        })? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Stops camera movement.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    fn move_stop(&mut self) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        })? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Moves the camera to an absolute position.
    ///
    /// # Arguments
    /// * `pan` - Pan position in VISCA units
    /// * `tilt` - Tilt position in VISCA units
    /// * `pan_speed` - Pan speed (0x01-0x18)
    /// * `tilt_speed` - Tilt speed (0x01-0x14)
    ///
    /// # Errors
    /// Returns `Error::InvalidParameter` if speeds are out of valid range,
    /// or `Error` if the command fails to send or the camera returns an error.
    fn move_absolute(
        &mut self,
        pan: i16,
        tilt: i16,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&PanTiltCommand::AbsolutePosition {
            pan,
            tilt,
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
        })? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Sets focus to auto mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    fn set_focus_auto(&mut self) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&FocusCommand::Auto)? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Sets focus to manual mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    fn set_focus_manual(&mut self) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&FocusCommand::Manual)? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Enables or disables backlight compensation.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    fn set_backlight(&mut self, enabled: bool) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&BacklightCommand { status: enabled })? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Sets exposure compensation value.
    ///
    /// # Arguments
    /// * `value` - Exposure compensation value (-7 to +7)
    ///
    /// # Errors
    /// Returns `Error::InvalidParameter` if value is not in the range -7 to +7,
    /// or `Error` if the command fails to send or the camera returns an error.
    fn set_exposure_compensation(&mut self, value: i8) -> Result<(), Error>
    where
        Self: Sized,
    {
        match self.execute_command(&ExposureCompensationCommand::Direct(
            ExposureCompensationLevel::new(value)?,
        ))? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Sets iris value directly.
    ///
    /// # Arguments
    /// * `value` - Iris value in VISCA units
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    fn set_iris(&mut self, value: u8) -> Result<(), Error>
    where
        Self: Sized,
    {
        use crate::types::IrisLevel;
        match self.execute_command(&IrisCommand::Direct(IrisLevel::new(value)?))? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Sets gain value directly.
    ///
    /// # Arguments
    /// * `value` - Gain value in VISCA units
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    fn set_gain(&mut self, value: u16) -> Result<(), Error>
    where
        Self: Sized,
    {
        use crate::types::GainValue;
        // Gain values are 0x00-0x07, so we need to ensure the u16 fits
        if value > 0x07 {
            return Err(Error::ParameterOutOfRange {
                parameter: "gain".to_string(),
                value: i32::from(value),
                min: 0x00,
                max: 0x07,
            });
        }
        #[allow(clippy::cast_possible_truncation)]
        let gain_value = GainValue::new(value as u8)?;
        match self.execute_command(&GainCommand::Direct(gain_value))? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blanket implementation for all types that implement Transport
impl<T: Transport> TransportExt for T {}
