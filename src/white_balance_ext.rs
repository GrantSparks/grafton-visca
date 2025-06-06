//! High-level extension trait for white balance control operations.

// Crate imports
use crate::{
    command::{
        BlueGainCommand, ColorTemperatureCommand, OnePushTriggerCommand, RedGainCommand,
        WhiteBalanceCommand, WhiteBalanceMode,
    },
    error::ViscaError,
    ViscaDevice, ViscaResponse,
};

/// Extension trait providing high-level white balance control methods.
pub trait ViscaWhiteBalanceExt: ViscaDevice {
    /// Set the white balance mode.
    ///
    /// # Arguments
    /// * `mode` - The white balance mode to set
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # use grafton_visca::command::white_balance::WhiteBalanceMode;
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
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
    fn set_white_balance_mode(&mut self, mode: WhiteBalanceMode) -> Result<(), ViscaError> {
        let command = WhiteBalanceCommand { mode };
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set the color temperature directly.
    ///
    /// # Arguments
    /// * `value` - Color temperature value (0x00=2500K to 0x37=8000K)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Set to approximately 5600K (daylight)
    /// client.set_color_temperature_direct(0x1C)?;
    ///
    /// // Set to approximately 3200K (tungsten)
    /// client.set_color_temperature_direct(0x0B)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_color_temperature_direct(&mut self, value: u16) -> Result<(), ViscaError> {
        let command = ColorTemperatureCommand::Direct(value);
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase color temperature.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.color_temperature_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn color_temperature_up(&mut self) -> Result<(), ViscaError> {
        let command = ColorTemperatureCommand::Up;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease color temperature.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.color_temperature_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn color_temperature_down(&mut self) -> Result<(), ViscaError> {
        let command = ColorTemperatureCommand::Down;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset color temperature.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.color_temperature_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn color_temperature_reset(&mut self) -> Result<(), ViscaError> {
        let command = ColorTemperatureCommand::Reset;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Trigger one-push white balance calibration.
    ///
    /// This will calibrate the white balance based on the current scene.
    /// Point the camera at a white or neutral gray surface before calling this.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Set to one-push mode first
    /// client.set_white_balance_mode(grafton_visca::command::white_balance::WhiteBalanceMode::OnePush)?;
    ///
    /// // Trigger calibration
    /// client.trigger_one_push_white_balance()?;
    /// # Ok(())
    /// # }
    /// ```
    fn trigger_one_push_white_balance(&mut self) -> Result<(), ViscaError> {
        let command = OnePushTriggerCommand;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Set manual white balance gain values.
    ///
    /// # Arguments
    /// * `r_gain` - Red gain value (0x00 to 0xFF)
    /// * `b_gain` - Blue gain value (0x00 to 0xFF)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
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
    fn set_manual_white_balance_gain(&mut self, r_gain: u8, b_gain: u8) -> Result<(), ViscaError> {
        // Set red gain
        let r_command = RedGainCommand::Direct(r_gain);
        match self.execute_command(&r_command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }

        // Set blue gain
        let b_command = BlueGainCommand::Direct(b_gain);
        match self.execute_command(&b_command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }

        Ok(())
    }

    /// Increase red gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.white_balance_red_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_up(&mut self) -> Result<(), ViscaError> {
        let command = RedGainCommand::Up;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease red gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.white_balance_red_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_down(&mut self) -> Result<(), ViscaError> {
        let command = RedGainCommand::Down;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset red gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.white_balance_red_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_reset(&mut self) -> Result<(), ViscaError> {
        let command = RedGainCommand::Reset;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Increase blue gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.white_balance_blue_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_up(&mut self) -> Result<(), ViscaError> {
        let command = BlueGainCommand::Up;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Decrease blue gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.white_balance_blue_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_down(&mut self) -> Result<(), ViscaError> {
        let command = BlueGainCommand::Down;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Reset blue gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// client.white_balance_blue_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_reset(&mut self) -> Result<(), ViscaError> {
        let command = BlueGainCommand::Reset;
        match self.execute_command(&command)? {
            ViscaResponse::Completion => {},
            ViscaResponse::Error(e) => return Err(e),
            _ => return Err(ViscaError::UnexpectedResponseType),
        }
        Ok(())
    }

    /// Convenience method to set common white balance presets.
    ///
    /// # Arguments
    /// * `preset` - A common lighting preset
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaWhiteBalanceExt, WhiteBalancePreset};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
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
    fn set_white_balance_preset(&mut self, preset: WhiteBalancePreset) -> Result<(), ViscaError> {
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

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: ViscaDevice> ViscaWhiteBalanceExt for T {}
