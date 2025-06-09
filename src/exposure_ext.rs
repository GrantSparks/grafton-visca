//! High-level extension trait for exposure control operations.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{
        AntiFlickerCommand, AntiFlickerMode, BacklightCommand, BrightCommand, ExposureCommand,
        ExposureCompensationCommand, ExposureCompensationLevel, ExposureMode, GainCommand,
        GainLimitCommand, IrisCommand, ShutterCommand,
    },
    error::Error as ViscaError,
    execute_command, Transport,
};

/// Extension trait providing high-level exposure control methods.
pub trait ViscaExposureExt: Transport {
    /// Set the exposure mode.
    ///
    /// # Arguments
    /// * `mode` - The exposure mode to set
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # use grafton_visca::command::exposure::ExposureMode;
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
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
        execute_command!(self, ExposureCommand { mode })
    }

    /// Set the exposure compensation level.
    ///
    /// # Arguments
    /// * `level` - Compensation level (-7 to +7)
    ///
    /// # Errors
    /// * `ViscaError::InvalidParameter` - Level is outside the valid range (-7 to +7)
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
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
        execute_command!(
            self,
            ExposureCompensationCommand::Direct(compensation_level)
        )
    }

    /// Enable or disable exposure compensation.
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable exposure compensation
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
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
        execute_command!(self, command)
    }

    /// Reset exposure compensation to 0.
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.reset_exposure_compensation()?;
    /// # Ok(())
    /// # }
    /// ```
    fn reset_exposure_compensation(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, ExposureCompensationCommand::Reset)
    }

    /// Increase exposure compensation by one step.
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.exposure_compensation_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn exposure_compensation_up(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, ExposureCompensationCommand::Up)
    }

    /// Decrease exposure compensation by one step.
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.exposure_compensation_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn exposure_compensation_down(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, ExposureCompensationCommand::Down)
    }

    /// Set the iris value.
    ///
    /// # Arguments
    /// * `value` - Iris value (0x00 to 0x0C)
    ///
    /// # Errors
    /// * `ViscaError::InvalidParameter` - Value is outside the valid range (0x00 to 0x0C)
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Set iris to F5.6
    /// client.set_iris(0x08)?;
    ///
    /// // Set iris to F1.8 (most open)
    /// client.set_iris(0x0C)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_iris(&mut self, value: u8) -> Result<(), ViscaError> {
        use crate::types::IrisLevel;
        execute_command!(self, IrisCommand::Direct(IrisLevel::new(value)?))
    }

    /// Increase iris opening (brighter).
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.iris_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn iris_up(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, IrisCommand::Up)
    }

    /// Decrease iris opening (darker).
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.iris_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn iris_down(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, IrisCommand::Down)
    }

    /// Reset iris to default.
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.iris_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn iris_reset(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, IrisCommand::Reset)
    }

    /// Set the shutter speed.
    ///
    /// # Arguments
    /// * `value` - Shutter speed value (0x00 to 0x15)
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Set shutter to 1/60
    /// client.set_shutter(0x08)?;
    ///
    /// // Set shutter to 1/30
    /// client.set_shutter(0x06)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_shutter(&mut self, value: u8) -> Result<(), ViscaError> {
        use crate::types::ShutterSpeed;
        execute_command!(
            self,
            ShutterCommand::Direct(ShutterSpeed::new(u16::from(value))?)
        )
    }

    /// Increase shutter speed (faster, darker).
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.shutter_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn shutter_up(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, ShutterCommand::Up)
    }

    /// Decrease shutter speed (slower, brighter).
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.shutter_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn shutter_down(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, ShutterCommand::Down)
    }

    /// Reset shutter to default.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.shutter_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn shutter_reset(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, ShutterCommand::Reset)
    }

    /// Set the gain value.
    ///
    /// # Arguments
    /// * `value` - Gain value (0x00 to 0x0F)
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Set minimum gain (0 dB)
    /// client.set_gain(0x00)?;
    ///
    /// // Set medium gain
    /// client.set_gain(0x08)?;
    ///
    /// // Set maximum gain (48 dB)
    /// client.set_gain(0x0F)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_gain(&mut self, value: u8) -> Result<(), ViscaError> {
        use crate::types::GainValue;
        execute_command!(self, GainCommand::Direct(GainValue::new(value)?))
    }

    /// Increase gain (brighter but more noise).
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.gain_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn gain_up(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, GainCommand::Up)
    }

    /// Decrease gain (darker but less noise).
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.gain_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn gain_down(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, GainCommand::Down)
    }

    /// Reset gain to default.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.gain_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn gain_reset(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, GainCommand::Reset)
    }

    /// Set the gain limit.
    ///
    /// # Arguments
    /// * `limit` - Maximum allowed gain value (0x04 to 0x0F)
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Limit gain to 24 dB
    /// client.set_gain_limit(0x08)?;
    ///
    /// // Allow full gain range
    /// client.set_gain_limit(0x0F)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_gain_limit(&mut self, limit: u8) -> Result<(), ViscaError> {
        use crate::types::GainLimit;
        execute_command!(
            self,
            GainLimitCommand {
                limit: GainLimit::new(limit)?
            }
        )
    }

    /// Set the brightness adjustment.
    ///
    /// # Arguments
    /// * `value` - Brightness value (0x00 to 0x17)
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Set neutral brightness
    /// client.set_brightness(0x0C)?;
    ///
    /// // Increase brightness
    /// client.set_brightness(0x10)?;
    ///
    /// // Decrease brightness
    /// client.set_brightness(0x08)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_brightness(&mut self, value: u8) -> Result<(), ViscaError> {
        use crate::types::BrightnessLevel;
        execute_command!(
            self,
            BrightCommand::Direct(BrightnessLevel::new(u16::from(value))?)
        )
    }

    /// Increase brightness.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.brightness_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn brightness_up(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, BrightCommand::Up)
    }

    /// Decrease brightness.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.brightness_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn brightness_down(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, BrightCommand::Down)
    }

    /// Reset brightness to default.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// client.brightness_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn brightness_reset(&mut self) -> Result<(), ViscaError> {
        execute_command!(self, BrightCommand::Reset)
    }

    /// Enable or disable backlight compensation.
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable backlight compensation
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Enable backlight compensation for subjects against bright backgrounds
    /// client.set_backlight(true)?;
    ///
    /// // Disable backlight compensation
    /// client.set_backlight(false)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_backlight(&mut self, enabled: bool) -> Result<(), ViscaError> {
        execute_command!(self, BacklightCommand { status: enabled })
    }

    /// Set the anti-flicker mode.
    ///
    /// Reduces flicker caused by artificial lighting that operates at
    /// different frequencies than the camera's frame rate.
    ///
    /// # Arguments
    /// * `mode` - The anti-flicker mode to apply
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt};
    /// # use grafton_visca::command::AntiFlickerMode;
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Disable anti-flicker
    /// client.set_anti_flicker(AntiFlickerMode::Off)?;
    ///
    /// // Enable 50Hz anti-flicker (for regions with 50Hz AC power)
    /// client.set_anti_flicker(AntiFlickerMode::Hz50)?;
    ///
    /// // Enable 60Hz anti-flicker (for regions with 60Hz AC power)
    /// client.set_anti_flicker(AntiFlickerMode::Hz60)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_anti_flicker(&mut self, mode: AntiFlickerMode) -> Result<(), ViscaError> {
        execute_command!(self, AntiFlickerCommand { mode })
    }

    /// Configure exposure settings for common scenarios.
    ///
    /// # Arguments
    /// * `preset` - The exposure preset to apply
    ///
    /// # Errors
    /// Returns `ViscaError` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, Transport, ViscaExposureExt, ExposurePreset};
    /// # use grafton_visca::command::exposure::ExposureMode;
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Configure for bright daylight
    /// client.configure_exposure_preset(ExposurePreset::BrightDaylight)?;
    ///
    /// // Configure for indoor lighting
    /// client.configure_exposure_preset(ExposurePreset::Indoor)?;
    ///
    /// // Configure for low light
    /// client.configure_exposure_preset(ExposurePreset::LowLight)?;
    /// # Ok(())
    /// # }
    /// ```
    fn configure_exposure_preset(&mut self, preset: ExposurePreset) -> Result<(), ViscaError> {
        match preset {
            ExposurePreset::BrightDaylight => {
                self.set_exposure_mode(ExposureMode::Auto)?;
                self.set_gain_limit(0x04)?; // Limit gain to reduce noise
                self.set_iris(0x08)?; // F5.6
                self.set_backlight(false)?;
            }
            ExposurePreset::Indoor => {
                self.set_exposure_mode(ExposureMode::Auto)?;
                self.set_gain_limit(0x08)?; // Allow moderate gain
                self.set_iris(0x0A)?; // F2.8
                self.set_backlight(false)?;
            }
            ExposurePreset::LowLight => {
                self.set_exposure_mode(ExposureMode::Auto)?;
                self.set_gain_limit(0x0F)?; // Allow full gain
                self.set_iris(0x0C)?; // F1.8 (most open)
                self.set_backlight(false)?;
            }
            ExposurePreset::Backlit => {
                self.set_exposure_mode(ExposureMode::Auto)?;
                self.set_gain_limit(0x08)?;
                self.set_backlight(true)?;
            }
            ExposurePreset::Manual => {
                self.set_exposure_mode(ExposureMode::Manual)?;
                // User will control individual settings
            }
        }
        Ok(())
    }
}

/// Common exposure presets for different scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExposurePreset {
    /// Bright outdoor daylight
    BrightDaylight,
    /// Indoor lighting
    Indoor,
    /// Low light conditions
    LowLight,
    /// Subject against bright background
    Backlit,
    /// Manual control
    Manual,
}

impl<T: Transport> ViscaExposureExt for T {}
