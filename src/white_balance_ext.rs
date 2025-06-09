//! High-level extension trait for white balance control operations.

// Crate imports
use crate::{
    command::{
        BlueGainCommand, BlueTuningCommand, ColorTemperatureCommand, OnePushTriggerCommand,
        RedGainCommand, RedTuningCommand, WhiteBalanceCommand, WhiteBalanceMode,
    },
    error::Error as Error,
    Response, Transport,
};

/// Extension trait providing high-level white balance control methods.
pub trait WhiteBalanceExt: Transport {
    /// Set the white balance mode.
    ///
    /// # Arguments
    /// * `mode` - The white balance mode to set
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # use grafton_visca::command::white_balance::WhiteBalanceMode;
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Set to auto white balance
    /// client.set_white_balance_mode(WhiteBalanceMode::Auto)?;
    ///
    /// // Set to indoor mode
    /// client.set_white_balance_mode(WhiteBalanceMode::Indoor)?;
    ///
    /// // Set to manual mode
    /// client.set_white_balance_mode(WhiteBalanceMode::Manual)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_white_balance_mode(&mut self, mode: WhiteBalanceMode) -> Result<(), Error> {
        let command = WhiteBalanceCommand { mode };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set the color temperature directly.
    ///
    /// # Arguments
    /// * `value` - Color temperature value (0x00=2500K to 0x37=8000K)
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Set to approximately 5600K (daylight)
    /// client.set_color_temperature_direct(0x1C)?;
    ///
    /// // Set to approximately 3200K (tungsten)
    /// client.set_color_temperature_direct(0x0B)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_color_temperature_direct(&mut self, value: u16) -> Result<(), Error> {
        let command = ColorTemperatureCommand::Direct(value);
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase color temperature.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// client.color_temperature_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn color_temperature_up(&mut self) -> Result<(), Error> {
        let command = ColorTemperatureCommand::Up;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease color temperature.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// client.color_temperature_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn color_temperature_down(&mut self) -> Result<(), Error> {
        let command = ColorTemperatureCommand::Down;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset color temperature.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// client.color_temperature_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn color_temperature_reset(&mut self) -> Result<(), Error> {
        let command = ColorTemperatureCommand::Reset;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Trigger one-push white balance calibration.
    ///
    /// This will calibrate the white balance based on the current scene.
    /// Point the camera at a white or neutral gray surface before calling this.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Set to one-push mode first
    /// client.set_white_balance_mode(grafton_visca::command::white_balance::WhiteBalanceMode::OnePush)?;
    ///
    /// // Trigger calibration
    /// client.trigger_one_push_white_balance()?;
    /// # Ok(())
    /// # }
    /// ```
    fn trigger_one_push_white_balance(&mut self) -> Result<(), Error> {
        let command = OnePushTriggerCommand;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set manual white balance gain values.
    ///
    /// # Arguments
    /// * `r_gain` - Red gain value (0x00 to 0xFF)
    /// * `b_gain` - Blue gain value (0x00 to 0xFF)
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Set to manual mode first
    /// client.set_white_balance_mode(grafton_visca::command::white_balance::WhiteBalanceMode::Manual)?;
    ///
    /// // Set neutral gains
    /// client.set_manual_white_balance_gain(0x80, 0x80)?;
    ///
    /// // Adjust for warmer tone
    /// client.set_manual_white_balance_gain(0x90, 0x70)?;
    ///
    /// // Adjust for cooler tone
    /// client.set_manual_white_balance_gain(0x70, 0x90)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_manual_white_balance_gain(&mut self, r_gain: u8, b_gain: u8) -> Result<(), Error> {
        // Set red gain
        let r_command = RedGainCommand::Direct(r_gain);
        match self.execute_command(&r_command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }

        // Set blue gain
        let b_command = BlueGainCommand::Direct(b_gain);
        match self.execute_command(&b_command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }

        Ok(())
    }

    /// Increase red gain in manual white balance mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// client.white_balance_red_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_up(&mut self) -> Result<(), Error> {
        let command = RedGainCommand::Up;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease red gain in manual white balance mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// client.white_balance_red_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_down(&mut self) -> Result<(), Error> {
        let command = RedGainCommand::Down;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset red gain in manual white balance mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// client.white_balance_red_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_reset(&mut self) -> Result<(), Error> {
        let command = RedGainCommand::Reset;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase blue gain in manual white balance mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// client.white_balance_blue_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_up(&mut self) -> Result<(), Error> {
        let command = BlueGainCommand::Up;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease blue gain in manual white balance mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// client.white_balance_blue_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_down(&mut self) -> Result<(), Error> {
        let command = BlueGainCommand::Down;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset blue gain in manual white balance mode.
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// client.white_balance_blue_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_reset(&mut self) -> Result<(), Error> {
        let command = BlueGainCommand::Reset;
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Fine-tune the red channel gain for white balance adjustment.
    ///
    /// This is typically used after setting a base white balance mode
    /// to make small corrections. The tuning range is from -10 to +10,
    /// where 0 is neutral.
    ///
    /// # Arguments
    /// * `level` - Red tuning level (-10 to +10, where 0 is neutral)
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Slightly increase red
    /// client.white_balance_red_tuning(2)?;
    ///
    /// // Slightly decrease red
    /// client.white_balance_red_tuning(-3)?;
    ///
    /// // Reset to neutral
    /// client.white_balance_red_tuning(0)?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_tuning(&mut self, level: i8) -> Result<(), Error> {
        let command = RedTuningCommand { level };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Fine-tune the blue channel gain for white balance adjustment.
    ///
    /// This is typically used after setting a base white balance mode
    /// to make small corrections. The tuning range is from -10 to +10,
    /// where 0 is neutral.
    ///
    /// # Arguments
    /// * `level` - Blue tuning level (-10 to +10, where 0 is neutral)
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Slightly increase blue
    /// client.white_balance_blue_tuning(2)?;
    ///
    /// // Slightly decrease blue
    /// client.white_balance_blue_tuning(-3)?;
    ///
    /// // Reset to neutral
    /// client.white_balance_blue_tuning(0)?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_tuning(&mut self, level: i8) -> Result<(), Error> {
        let command = BlueTuningCommand { level };
        match self.execute_command(&command)? {
            Response::Completion => {}
            Response::Error(e) => return Err(e),
            _ => return Err(Error::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Convenience method to set common white balance presets.
    ///
    /// # Arguments
    /// * `preset` - A common lighting preset
    ///
    /// # Errors
    /// Returns `Error` if the command fails to send or the camera returns an error.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, WhiteBalanceExt, WhiteBalancePreset};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Set for daylight
    /// client.set_white_balance_preset(WhiteBalancePreset::Daylight)?;
    ///
    /// // Set for tungsten lighting
    /// client.set_white_balance_preset(WhiteBalancePreset::Tungsten)?;
    ///
    /// // Set for fluorescent lighting
    /// client.set_white_balance_preset(WhiteBalancePreset::Fluorescent)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_white_balance_preset(&mut self, preset: WhiteBalancePreset) -> Result<(), Error> {
        match preset {
            WhiteBalancePreset::Auto => self.set_white_balance_mode(WhiteBalanceMode::Auto),
            WhiteBalancePreset::Daylight => {
                self.set_white_balance_mode(WhiteBalanceMode::ColorTemperature)?;
                self.set_color_temperature_direct(0x1C) // ~5600K
            }
            WhiteBalancePreset::Cloudy => {
                self.set_white_balance_mode(WhiteBalanceMode::ColorTemperature)?;
                self.set_color_temperature_direct(0x24) // ~6500K
            }
            WhiteBalancePreset::Tungsten => {
                self.set_white_balance_mode(WhiteBalanceMode::ColorTemperature)?;
                self.set_color_temperature_direct(0x0B) // ~3200K
            }
            WhiteBalancePreset::Fluorescent => {
                self.set_white_balance_mode(WhiteBalanceMode::ColorTemperature)?;
                self.set_color_temperature_direct(0x15) // ~4000K
            }
            WhiteBalancePreset::Indoor => self.set_white_balance_mode(WhiteBalanceMode::Indoor),
            WhiteBalancePreset::Outdoor => self.set_white_balance_mode(WhiteBalanceMode::Outdoor),
        }
    }
}

/// Common white balance presets for convenience.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteBalancePreset {
    /// Automatic white balance
    Auto,
    /// Daylight (5600K)
    Daylight,
    /// Cloudy (6500K)
    Cloudy,
    /// Tungsten/Incandescent (3200K)
    Tungsten,
    /// Fluorescent (4000K)
    Fluorescent,
    /// Indoor mode
    Indoor,
    /// Outdoor mode
    Outdoor,
}

/// Implement the trait for all types that implement `Transport`
impl<T: Transport> WhiteBalanceExt for T {}
