//! Exposure methods for cameras using the new GAT architecture.

use crate::{camera::Camera, Error};

/// Exposure operations (async).
pub trait ExposureOps: Sized {
    /// Set auto exposure mode.
    async fn exposure_auto(&self) -> Result<(), Error>;

    /// Set manual exposure mode.
    async fn exposure_manual(&self) -> Result<(), Error>;

    /// Set iris level.
    async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error>;

    /// Set brightness level.
    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error>;

    /// Set backlight compensation.
    async fn set_backlight(&self, enabled: bool) -> Result<(), Error>;

    /// Set gain value.
    async fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error>;

    /// Set gain limit.
    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set dynamic range level.
    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set color temperature.
    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error>;
}

/// Exposure operations (blocking).
pub trait ExposureOpsBlocking: Sized {
    /// Set auto exposure mode.
    fn exposure_auto(&self) -> Result<(), Error>;

    /// Set manual exposure mode.
    fn exposure_manual(&self) -> Result<(), Error>;

    /// Set iris level.
    fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error>;

    /// Set brightness level.
    fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error>;

    /// Set backlight compensation.
    fn set_backlight(&self, enabled: bool) -> Result<(), Error>;

    /// Set gain value.
    fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error>;

    /// Set gain limit.
    fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set dynamic range level.
    fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set color temperature.
    fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error>;
}

// Async implementation
impl ExposureOps for Camera {
    async fn exposure_auto(&self) -> Result<(), Error> {
        use crate::command::exposure::{ExposureCommand, ExposureMode};

        let command = ExposureCommand {
            mode: ExposureMode::Auto,
        };
        self.send_command(&command).await?;
        Ok(())
    }

    async fn exposure_manual(&self) -> Result<(), Error> {
        use crate::command::exposure::{ExposureCommand, ExposureMode};

        let command = ExposureCommand {
            mode: ExposureMode::Manual,
        };
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        use crate::command::exposure::Iris;

        let command = Iris::SetAperture(level);
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        use crate::command::exposure::Bright;

        let command = Bright::SetLevel(level);
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        use crate::command::image::BacklightCommand;

        let command = BacklightCommand::new(enabled);
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
        use crate::command::gain::Gain;

        let command = Gain::SetValue(gain);
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        use crate::command::gain::GainLimitCommand;

        let command = GainLimitCommand::new(limit);
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        use crate::command::exposure::DynamicRange;

        let command = DynamicRange::SetLevel(level);
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error> {
        use crate::command::color::ColorTemperature;

        let command = ColorTemperature::SetTemperature(temp);
        self.send_command(&command).await?;
        Ok(())
    }
}

// Blocking implementation
impl ExposureOpsBlocking for Camera {
    fn exposure_auto(&self) -> Result<(), Error> {
        use crate::command::exposure::{ExposureCommand, ExposureMode};

        let command = ExposureCommand {
            mode: ExposureMode::Auto,
        };
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn exposure_manual(&self) -> Result<(), Error> {
        use crate::command::exposure::{ExposureCommand, ExposureMode};

        let command = ExposureCommand {
            mode: ExposureMode::Manual,
        };
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        use crate::command::exposure::Iris;

        let command = Iris::SetAperture(level);
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        use crate::command::exposure::Bright;

        let command = Bright::SetLevel(level);
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        use crate::command::image::BacklightCommand;

        let command = BacklightCommand::new(enabled);
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
        use crate::command::gain::Gain;

        let command = Gain::SetValue(gain);
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        use crate::command::gain::GainLimitCommand;

        let command = GainLimitCommand::new(limit);
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        use crate::command::exposure::DynamicRange;

        let command = DynamicRange::SetLevel(level);
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error> {
        use crate::command::color::ColorTemperature;

        let command = ColorTemperature::SetTemperature(temp);
        self.send_command_blocking(&command)?;
        Ok(())
    }
}
