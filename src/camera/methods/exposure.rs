//! Exposure methods for cameras using the new GAT architecture.

use crate::{camera::unified::Camera, Error};

/// Exposure operations.
pub trait ExposureOps: Sized {
    /// Set auto exposure mode.
    #[cfg(feature = "tokio")]
    async fn exposure_auto(&self) -> Result<(), Error>;

    /// Set auto exposure mode. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn exposure_auto_blocking(&mut self) -> Result<(), Error>;

    /// Set manual exposure mode.
    #[cfg(feature = "tokio")]
    async fn exposure_manual(&self) -> Result<(), Error>;

    /// Set manual exposure mode. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn exposure_manual_blocking(&mut self) -> Result<(), Error>;

    /// Set iris level.
    #[cfg(feature = "tokio")]
    async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error>;

    /// Set iris level. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_iris_blocking(&mut self, level: crate::types::IrisLevel) -> Result<(), Error>;

    /// Set brightness level.
    #[cfg(feature = "tokio")]
    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error>;

    /// Set brightness level. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_brightness_blocking(
        &mut self,
        level: crate::types::BrightnessLevel,
    ) -> Result<(), Error>;

    /// Set backlight compensation.
    #[cfg(feature = "tokio")]
    async fn set_backlight(&self, enabled: bool) -> Result<(), Error>;

    /// Set backlight compensation. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_backlight_blocking(&mut self, enabled: bool) -> Result<(), Error>;

    /// Set gain value.
    #[cfg(feature = "tokio")]
    async fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error>;

    /// Set gain value. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_gain_blocking(&mut self, gain: crate::types::GainLevel) -> Result<(), Error>;

    /// Set gain limit.
    #[cfg(feature = "tokio")]
    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set gain limit. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_gain_limit_blocking(&mut self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set dynamic range level.
    #[cfg(feature = "tokio")]
    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set dynamic range level. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_dynamic_range_blocking(
        &mut self,
        level: crate::types::DynamicRangeLevel,
    ) -> Result<(), Error>;

    /// Set color temperature.
    #[cfg(feature = "tokio")]
    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error>;

    /// Set color temperature. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_color_temperature_blocking(
        &mut self,
        temp: crate::types::ColorTemp,
    ) -> Result<(), Error>;
}

impl ExposureOps for Camera {
    #[cfg(feature = "tokio")]
    async fn exposure_auto(&self) -> Result<(), Error> {
        use crate::command::exposure::{ExposureCommand, ExposureMode};

        let command = ExposureCommand {
            mode: ExposureMode::Auto,
        };
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn exposure_auto_blocking(&mut self) -> Result<(), Error> {
        use crate::command::exposure::{ExposureCommand, ExposureMode};

        let command = ExposureCommand {
            mode: ExposureMode::Auto,
        };
        self.send_command_blocking(&command)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn exposure_manual(&self) -> Result<(), Error> {
        use crate::command::exposure::{ExposureCommand, ExposureMode};

        let command = ExposureCommand {
            mode: ExposureMode::Manual,
        };
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn exposure_manual_blocking(&mut self) -> Result<(), Error> {
        use crate::command::exposure::{ExposureCommand, ExposureMode};

        let command = ExposureCommand {
            mode: ExposureMode::Manual,
        };
        self.send_command_blocking(&command)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        use crate::command::exposure::Iris;

        let command = Iris::SetAperture(level);
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_iris_blocking(&mut self, level: crate::types::IrisLevel) -> Result<(), Error> {
        use crate::command::exposure::Iris;

        let command = Iris::SetAperture(level);
        self.send_command_blocking(&command)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        use crate::command::exposure::Bright;

        let command = Bright::SetLevel(level);
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_brightness_blocking(
        &mut self,
        level: crate::types::BrightnessLevel,
    ) -> Result<(), Error> {
        use crate::command::exposure::Bright;

        let command = Bright::SetLevel(level);
        self.send_command_blocking(&command)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        use crate::command::image::BacklightCommand;

        let command = BacklightCommand::new(enabled);
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_backlight_blocking(&mut self, enabled: bool) -> Result<(), Error> {
        use crate::command::image::BacklightCommand;

        let command = BacklightCommand::new(enabled);
        self.send_command_blocking(&command)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
        use crate::command::gain::Gain;

        let command = Gain::SetValue(gain);
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_gain_blocking(&mut self, gain: crate::types::GainLevel) -> Result<(), Error> {
        use crate::command::gain::Gain;

        let command = Gain::SetValue(gain);
        self.send_command_blocking(&command)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        use crate::command::gain::GainLimitCommand;

        let command = GainLimitCommand::new(limit);
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_gain_limit_blocking(&mut self, limit: crate::types::GainLimit) -> Result<(), Error> {
        use crate::command::gain::GainLimitCommand;

        let command = GainLimitCommand::new(limit);
        self.send_command_blocking(&command)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        use crate::command::exposure::DynamicRange;

        let command = DynamicRange::SetLevel(level);
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_dynamic_range_blocking(
        &mut self,
        level: crate::types::DynamicRangeLevel,
    ) -> Result<(), Error> {
        use crate::command::exposure::DynamicRange;

        let command = DynamicRange::SetLevel(level);
        self.send_command_blocking(&command)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error> {
        use crate::command::color::ColorTemperature;

        let command = ColorTemperature::SetTemperature(temp);
        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_color_temperature_blocking(
        &mut self,
        temp: crate::types::ColorTemp,
    ) -> Result<(), Error> {
        use crate::command::color::ColorTemperature;

        let command = ColorTemperature::SetTemperature(temp);
        self.send_command_blocking(&command)?;
        Ok(())
    }
}
