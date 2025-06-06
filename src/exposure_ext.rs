//! High-level extension trait for exposure control operations.

// Crate imports
use crate::{
    command::{
        BacklightCommand, BrightCommand, ExposureCommand, ExposureCompensationCommand,
        ExposureCompensationLevel, ExposureMode, GainCommand, GainLimitCommand, IrisCommand,
        ShutterCommand,
    },
    error::ViscaError,
    ViscaDevice, ViscaResponse,
};

/// Extension trait providing high-level exposure control methods.
pub trait ViscaExposureExt: ViscaDevice {
    /// Set the exposure mode.
    ///
    /// # Arguments
    /// * `mode` - The exposure mode to set
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # use grafton_visca::command::exposure::ExposureMode;
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Set to auto exposure
    /// client.set_exposure_mode(ExposureMode::Auto)?;
    ///
    /// // Set to manual exposure
    /// client.set_exposure_mode(ExposureMode::Manual)?;
    ///
    /// // Set to shutter priority
    /// client.set_exposure_mode(ExposureMode::Shutter)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_exposure_mode(&mut self, mode: ExposureMode) -> Result<(), ViscaError> {
        let command = ExposureCommand { mode };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set the exposure compensation level.
    ///
    /// # Arguments
    /// * `level` - Compensation level (-7 to +7)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Increase exposure by 2 stops
    /// client.set_exposure_compensation(2)?;
    ///
    /// // Decrease exposure by 1 stop
    /// client.set_exposure_compensation(-1)?;
    ///
    /// // Reset to no compensation
    /// client.set_exposure_compensation(0)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_exposure_compensation(&mut self, level: i8) -> Result<(), ViscaError> {
        let compensation_level = ExposureCompensationLevel::new(level)?;
        let command = ExposureCompensationCommand::Direct(compensation_level);
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Enable or disable exposure compensation.
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable exposure compensation
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Enable exposure compensation
    /// client.set_exposure_compensation_enabled(true)?;
    ///
    /// // Apply compensation
    /// client.set_exposure_compensation(2)?;
    ///
    /// // Disable exposure compensation
    /// client.set_exposure_compensation_enabled(false)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_exposure_compensation_enabled(&mut self, enabled: bool) -> Result<(), ViscaError> {
        let command = if enabled {
            ExposureCompensationCommand::On
        } else {
            ExposureCompensationCommand::Off
        };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset exposure compensation to 0.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.reset_exposure_compensation()?;
    /// # Ok(())
    /// # }
    /// ```
    fn reset_exposure_compensation(&mut self) -> Result<(), ViscaError> {
        let command = ExposureCompensationCommand::Reset;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase exposure compensation by one step.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.exposure_compensation_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn exposure_compensation_up(&mut self) -> Result<(), ViscaError> {
        let command = ExposureCompensationCommand::Up;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease exposure compensation by one step.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.exposure_compensation_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn exposure_compensation_down(&mut self) -> Result<(), ViscaError> {
        let command = ExposureCompensationCommand::Down;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set the iris value.
    ///
    /// # Arguments
    /// * `value` - Iris value (0x00 to 0x0C)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Set iris to F5.6
    /// client.set_iris(0x08)?;
    ///
    /// // Set iris to F1.8 (most open)
    /// client.set_iris(0x0C)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_iris(&mut self, value: u8) -> Result<(), ViscaError> {
        let command = IrisCommand::Direct(value);
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase iris opening (brighter).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.iris_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn iris_up(&mut self) -> Result<(), ViscaError> {
        let command = IrisCommand::Up;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease iris opening (darker).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.iris_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn iris_down(&mut self) -> Result<(), ViscaError> {
        let command = IrisCommand::Down;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset iris to default.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.iris_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn iris_reset(&mut self) -> Result<(), ViscaError> {
        let command = IrisCommand::Reset;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set the shutter speed.
    ///
    /// # Arguments
    /// * `value` - The shutter speed value (0x01 to 0x11)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Set to 1/60
    /// client.set_shutter_speed(0x06)?;
    ///
    /// // Set to 1/500  
    /// client.set_shutter_speed(0x0C)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_shutter_speed(&mut self, value: u16) -> Result<(), ViscaError> {
        let command = ShutterCommand::Direct(value);
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase shutter speed (faster/darker).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.shutter_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn shutter_up(&mut self) -> Result<(), ViscaError> {
        let command = ShutterCommand::Up;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease shutter speed (slower/brighter).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.shutter_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn shutter_down(&mut self) -> Result<(), ViscaError> {
        let command = ShutterCommand::Down;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset shutter speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.shutter_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn shutter_reset(&mut self) -> Result<(), ViscaError> {
        let command = ShutterCommand::Reset;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set the gain value.
    ///
    /// # Arguments
    /// * `gain` - Gain value (0x00 to 0x07)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Set minimal gain
    /// client.set_gain(0x00)?;
    ///
    /// // Set moderate gain
    /// client.set_gain(0x04)?;
    ///
    /// // Set maximum gain
    /// client.set_gain(0x07)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_gain(&mut self, gain: u16) -> Result<(), ViscaError> {
        let command = GainCommand::Direct(gain);
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set the gain limit.
    ///
    /// # Arguments
    /// * `limit` - The gain limit (0x0 to 0xF)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Limit gain to low values
    /// client.set_gain_limit(0x3)?;
    ///
    /// // Allow maximum gain
    /// client.set_gain_limit(0xF)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_gain_limit(&mut self, limit: u8) -> Result<(), ViscaError> {
        let command = GainLimitCommand { limit };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase gain (brighter).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.gain_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn gain_up(&mut self) -> Result<(), ViscaError> {
        let command = GainCommand::Up;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease gain (darker).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.gain_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn gain_down(&mut self) -> Result<(), ViscaError> {
        let command = GainCommand::Down;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset gain.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.gain_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn gain_reset(&mut self) -> Result<(), ViscaError> {
        let command = GainCommand::Reset;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set the brightness level.
    ///
    /// # Arguments
    /// * `brightness` - Brightness level (0x00 to 0x11)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Set minimal brightness
    /// client.set_brightness(0x00)?;
    ///
    /// // Set default brightness
    /// client.set_brightness(0x08)?;
    ///
    /// // Set maximum brightness
    /// client.set_brightness(0x11)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_brightness(&mut self, brightness: u16) -> Result<(), ViscaError> {
        let command = BrightCommand::Direct(brightness);
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase brightness.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.brightness_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn brightness_up(&mut self) -> Result<(), ViscaError> {
        let command = BrightCommand::Up;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease brightness.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.brightness_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn brightness_down(&mut self) -> Result<(), ViscaError> {
        let command = BrightCommand::Down;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset brightness.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.brightness_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn brightness_reset(&mut self) -> Result<(), ViscaError> {
        let command = BrightCommand::Reset;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Enable or disable backlight compensation.
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable backlight compensation
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaExposureExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Enable backlight compensation
    /// client.set_backlight_compensation(true)?;
    ///
    /// // Disable backlight compensation
    /// client.set_backlight_compensation(false)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_backlight_compensation(&mut self, enabled: bool) -> Result<(), ViscaError> {
        let command = BacklightCommand { status: enabled };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {}
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: ViscaDevice> ViscaExposureExt for T {}
