//! High-level extension trait for white balance control operations.

#![allow(deprecated)]

// Crate imports
use crate::{
    command::{
        BlueGainCommand, ColorTemperatureCommand, OnePushTriggerCommand, RedGainCommand,
        WhiteBalanceCommand, WhiteBalanceMode,
    },
    error::ViscaError,
    transport_ext::ViscaTransportExt,
};

/// Extension trait providing high-level white balance control methods.
pub trait ViscaWhiteBalanceExt: ViscaTransportExt {
    /// Set the white balance mode.
    ///
    /// # Arguments
    /// * `mode` - The white balance mode to set
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # use grafton_visca::command::white_balance::WhiteBalanceMode;
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set to auto white balance
    /// transport.set_white_balance_mode(WhiteBalanceMode::Auto)?;
    ///
    /// // Set to indoor mode
    /// transport.set_white_balance_mode(WhiteBalanceMode::Indoor)?;
    ///
    /// // Set to manual mode
    /// transport.set_white_balance_mode(WhiteBalanceMode::Manual)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_white_balance_mode(&mut self, mode: WhiteBalanceMode) -> Result<(), ViscaError> {
        let command = WhiteBalanceCommand { mode };
        self.send_command(&command)?;
        Ok(())
    }

    /// Set the color temperature directly.
    ///
    /// # Arguments
    /// * `value` - Color temperature value (0x00=2500K to 0x37=8000K)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set to approximately 5600K (daylight)
    /// transport.set_color_temperature_direct(0x1C)?;
    ///
    /// // Set to approximately 3200K (tungsten)
    /// transport.set_color_temperature_direct(0x0B)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_color_temperature_direct(&mut self, value: u16) -> Result<(), ViscaError> {
        let command = ColorTemperatureCommand::Direct(value);
        self.send_command(&command)?;
        Ok(())
    }

    /// Increase color temperature.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.color_temperature_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn color_temperature_up(&mut self) -> Result<(), ViscaError> {
        let command = ColorTemperatureCommand::Up;
        self.send_command(&command)?;
        Ok(())
    }

    /// Decrease color temperature.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.color_temperature_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn color_temperature_down(&mut self) -> Result<(), ViscaError> {
        let command = ColorTemperatureCommand::Down;
        self.send_command(&command)?;
        Ok(())
    }

    /// Reset color temperature.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.color_temperature_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn color_temperature_reset(&mut self) -> Result<(), ViscaError> {
        let command = ColorTemperatureCommand::Reset;
        self.send_command(&command)?;
        Ok(())
    }

    /// Trigger one-push white balance calibration.
    ///
    /// This will calibrate the white balance based on the current scene.
    /// Point the camera at a white or neutral gray surface before calling this.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set to one-push mode first
    /// transport.set_white_balance_mode(grafton_visca::command::white_balance::WhiteBalanceMode::OnePush)?;
    ///
    /// // Trigger calibration
    /// transport.trigger_one_push_white_balance()?;
    /// # Ok(())
    /// # }
    /// ```
    fn trigger_one_push_white_balance(&mut self) -> Result<(), ViscaError> {
        let command = OnePushTriggerCommand;
        self.send_command(&command)?;
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
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set to manual mode first
    /// transport.set_white_balance_mode(grafton_visca::command::white_balance::WhiteBalanceMode::Manual)?;
    ///
    /// // Set neutral gains
    /// transport.set_manual_white_balance_gain(0x80, 0x80)?;
    ///
    /// // Adjust for warmer tone
    /// transport.set_manual_white_balance_gain(0x90, 0x70)?;
    ///
    /// // Adjust for cooler tone
    /// transport.set_manual_white_balance_gain(0x70, 0x90)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_manual_white_balance_gain(&mut self, r_gain: u8, b_gain: u8) -> Result<(), ViscaError> {
        // Set red gain
        let r_command = RedGainCommand::Direct(r_gain);
        self.send_command(&r_command)?;

        // Set blue gain
        let b_command = BlueGainCommand::Direct(b_gain);
        self.send_command(&b_command)?;

        Ok(())
    }

    /// Increase red gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.white_balance_red_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_up(&mut self) -> Result<(), ViscaError> {
        let command = RedGainCommand::Up;
        self.send_command(&command)?;
        Ok(())
    }

    /// Decrease red gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.white_balance_red_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_down(&mut self) -> Result<(), ViscaError> {
        let command = RedGainCommand::Down;
        self.send_command(&command)?;
        Ok(())
    }

    /// Reset red gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.white_balance_red_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_red_reset(&mut self) -> Result<(), ViscaError> {
        let command = RedGainCommand::Reset;
        self.send_command(&command)?;
        Ok(())
    }

    /// Increase blue gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.white_balance_blue_up()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_up(&mut self) -> Result<(), ViscaError> {
        let command = BlueGainCommand::Up;
        self.send_command(&command)?;
        Ok(())
    }

    /// Decrease blue gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.white_balance_blue_down()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_down(&mut self) -> Result<(), ViscaError> {
        let command = BlueGainCommand::Down;
        self.send_command(&command)?;
        Ok(())
    }

    /// Reset blue gain in manual white balance mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// transport.white_balance_blue_reset()?;
    /// # Ok(())
    /// # }
    /// ```
    fn white_balance_blue_reset(&mut self) -> Result<(), ViscaError> {
        let command = BlueGainCommand::Reset;
        self.send_command(&command)?;
        Ok(())
    }

    /// Convenience method to set common white balance presets.
    ///
    /// # Arguments
    /// * `preset` - A common lighting preset
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaWhiteBalanceExt, WhiteBalancePreset};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set for daylight
    /// transport.set_white_balance_preset(WhiteBalancePreset::Daylight)?;
    ///
    /// // Set for tungsten lighting
    /// transport.set_white_balance_preset(WhiteBalancePreset::Tungsten)?;
    ///
    /// // Set for fluorescent lighting
    /// transport.set_white_balance_preset(WhiteBalancePreset::Fluorescent)?;
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
impl<T: ViscaTransportExt> ViscaWhiteBalanceExt for T {}
