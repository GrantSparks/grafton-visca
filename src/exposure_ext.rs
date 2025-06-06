//! High-level extension trait for exposure control operations.

// Crate imports
use crate::{
    command::{
        BacklightCommand, BrightCommand, ExposureCommand, ExposureCompensationCommand,
        ExposureCompensationLevel, ExposureMode, GainCommand, GainLimitCommand, IrisCommand,
        ShutterCommand,
    },
    error::ViscaError,
    transport_ext::ViscaTransportExt,
};

/// Extension trait providing high-level exposure control methods.
pub trait ViscaExposureExt: ViscaTransportExt {
    /// Set the exposure mode.
    ///
    /// # Arguments
    /// * `mode` - The exposure mode to set
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # use grafton_visca::command::exposure::ExposureMode;
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set to auto exposure
    /// transport.set_exposure_mode(ExposureMode::Auto)?;
    ///
    /// // Set to manual exposure
    /// transport.set_exposure_mode(ExposureMode::Manual)?;
    ///
    /// // Set to shutter priority
    /// transport.set_exposure_mode(ExposureMode::Shutter)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_exposure_mode(&mut self, mode: ExposureMode) -> Result<(), ViscaError> {
        let command = ExposureCommand { mode };
        self.send_command(&command)?;
        Ok(())
    }

    /// Set the exposure compensation level.
    ///
    /// # Arguments
    /// * `level` - Compensation level (-7 to +7)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Increase exposure by 2 stops
    /// transport.set_exposure_compensation(2)?;
    ///
    /// // Decrease exposure by 1 stop
    /// transport.set_exposure_compensation(-1)?;
    ///
    /// // Reset to no compensation
    /// transport.set_exposure_compensation(0)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_exposure_compensation(&mut self, level: i8) -> Result<(), ViscaError> {
        let compensation_level = ExposureCompensationLevel::new(level)?;
        let command = ExposureCompensationCommand::Direct(compensation_level);
        self.send_command(&command)?;
        Ok(())
    }

    /// Enable or disable exposure compensation.
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable exposure compensation
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Enable exposure compensation
    /// transport.set_exposure_compensation_enabled(true)?;
    ///
    /// // Apply compensation
    /// transport.set_exposure_compensation(2)?;
    ///
    /// // Disable exposure compensation
    /// transport.set_exposure_compensation_enabled(false)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_exposure_compensation_enabled(&mut self, enabled: bool) -> Result<(), ViscaError> {
        let command = if enabled {
            ExposureCompensationCommand::On
        } else {
            ExposureCompensationCommand::Off
        };
        self.send_command(&command)?;
        Ok(())
    }

    /// Reset exposure compensation to 0.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.reset_exposure_compensation()?;
    /// # Ok(())
    /// # }
    /// ```
    fn reset_exposure_compensation(&mut self) -> Result<(), ViscaError> {
        let command = ExposureCompensationCommand::Reset;
        self.send_command(&command)?;
        Ok(())
    }

    /// Increase exposure compensation by one step.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.exposure_compensation_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn exposure_compensation_up(&mut self) -> Result<(), ViscaError> {
        let command = ExposureCompensationCommand::Up;
        self.send_command(&command)?;
        Ok(())
    }

    /// Decrease exposure compensation by one step.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.exposure_compensation_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn exposure_compensation_down(&mut self) -> Result<(), ViscaError> {
        let command = ExposureCompensationCommand::Down;
        self.send_command(&command)?;
        Ok(())
    }

    /// Set the iris value.
    ///
    /// # Arguments
    /// * `value` - Iris value (0x00 to 0x0C)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set iris to F5.6
    /// transport.set_iris(0x08)?;
    ///
    /// // Set iris to F1.8 (most open)
    /// transport.set_iris(0x0C)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_iris(&mut self, value: u8) -> Result<(), ViscaError> {
        let command = IrisCommand::Direct(value);
        self.send_command(&command)?;
        Ok(())
    }

    /// Increase iris opening (brighter).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.iris_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn iris_up(&mut self) -> Result<(), ViscaError> {
        let command = IrisCommand::Up;
        self.send_command(&command)?;
        Ok(())
    }

    /// Decrease iris opening (darker).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.iris_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn iris_down(&mut self) -> Result<(), ViscaError> {
        let command = IrisCommand::Down;
        self.send_command(&command)?;
        Ok(())
    }

    /// Reset iris to default.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.iris_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn iris_reset(&mut self) -> Result<(), ViscaError> {
        let command = IrisCommand::Reset;
        self.send_command(&command)?;
        Ok(())
    }

    /// Set the shutter speed.
    ///
    /// # Arguments
    /// * `value` - The shutter speed value (0x01 to 0x11)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set to 1/60
    /// transport.set_shutter_speed(0x06)?;
    ///
    /// // Set to 1/500  
    /// transport.set_shutter_speed(0x0C)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_shutter_speed(&mut self, value: u16) -> Result<(), ViscaError> {
        let command = ShutterCommand::Direct(value);
        self.send_command(&command)?;
        Ok(())
    }

    /// Increase shutter speed (faster/darker).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.shutter_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn shutter_up(&mut self) -> Result<(), ViscaError> {
        let command = ShutterCommand::Up;
        self.send_command(&command)?;
        Ok(())
    }

    /// Decrease shutter speed (slower/brighter).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.shutter_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn shutter_down(&mut self) -> Result<(), ViscaError> {
        let command = ShutterCommand::Down;
        self.send_command(&command)?;
        Ok(())
    }

    /// Reset shutter speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.shutter_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn shutter_reset(&mut self) -> Result<(), ViscaError> {
        let command = ShutterCommand::Reset;
        self.send_command(&command)?;
        Ok(())
    }

    /// Set the gain value.
    ///
    /// # Arguments
    /// * `gain` - Gain value (0x00 to 0x07)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set minimal gain
    /// transport.set_gain(0x00)?;
    ///
    /// // Set moderate gain
    /// transport.set_gain(0x04)?;
    ///
    /// // Set maximum gain
    /// transport.set_gain(0x07)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_gain(&mut self, gain: u16) -> Result<(), ViscaError> {
        let command = GainCommand::Direct(gain);
        self.send_command(&command)?;
        Ok(())
    }

    /// Set the gain limit.
    ///
    /// # Arguments
    /// * `limit` - The gain limit (0x0 to 0xF)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Limit gain to low values
    /// transport.set_gain_limit(0x3)?;
    ///
    /// // Allow maximum gain
    /// transport.set_gain_limit(0xF)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_gain_limit(&mut self, limit: u8) -> Result<(), ViscaError> {
        let command = GainLimitCommand { limit };
        self.send_command(&command)?;
        Ok(())
    }

    /// Increase gain (brighter).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.gain_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn gain_up(&mut self) -> Result<(), ViscaError> {
        let command = GainCommand::Up;
        self.send_command(&command)?;
        Ok(())
    }

    /// Decrease gain (darker).
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.gain_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn gain_down(&mut self) -> Result<(), ViscaError> {
        let command = GainCommand::Down;
        self.send_command(&command)?;
        Ok(())
    }

    /// Reset gain.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.gain_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn gain_reset(&mut self) -> Result<(), ViscaError> {
        let command = GainCommand::Reset;
        self.send_command(&command)?;
        Ok(())
    }

    /// Set the brightness level.
    ///
    /// # Arguments
    /// * `brightness` - Brightness level (0x00 to 0x11)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set minimal brightness
    /// transport.set_brightness(0x00)?;
    ///
    /// // Set default brightness
    /// transport.set_brightness(0x08)?;
    ///
    /// // Set maximum brightness
    /// transport.set_brightness(0x11)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_brightness(&mut self, brightness: u16) -> Result<(), ViscaError> {
        let command = BrightCommand::Direct(brightness);
        self.send_command(&command)?;
        Ok(())
    }

    /// Increase brightness.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.brightness_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn brightness_up(&mut self) -> Result<(), ViscaError> {
        let command = BrightCommand::Up;
        self.send_command(&command)?;
        Ok(())
    }

    /// Decrease brightness.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.brightness_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn brightness_down(&mut self) -> Result<(), ViscaError> {
        let command = BrightCommand::Down;
        self.send_command(&command)?;
        Ok(())
    }

    /// Reset brightness.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.brightness_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn brightness_reset(&mut self) -> Result<(), ViscaError> {
        let command = BrightCommand::Reset;
        self.send_command(&command)?;
        Ok(())
    }

    /// Enable or disable backlight compensation.
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable backlight compensation
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaExposureExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Enable backlight compensation
    /// transport.set_backlight_compensation(true)?;
    ///
    /// // Disable backlight compensation
    /// transport.set_backlight_compensation(false)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_backlight_compensation(&mut self, enabled: bool) -> Result<(), ViscaError> {
        let command = BacklightCommand { status: enabled };
        self.send_command(&command)?;
        Ok(())
    }
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: ViscaTransportExt> ViscaExposureExt for T {}
