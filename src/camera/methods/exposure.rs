//! Exposure methods for cameras that support exposure control.

use crate::camera::Camera;
use crate::capabilities::{Exposure, ProfileMetadata};
use crate::command::Command;
use crate::Error;
use grafton_visca_macros::dual_native_method;

/// Extension trait that adds exposure methods to cameras.
#[allow(async_fn_in_trait)]
pub trait ExposureMethodsExt {
    /// Set auto exposure mode.
    #[cfg(not(feature = "async"))]
    fn exposure_auto(&mut self) -> Result<(), Error>;

    /// Set auto exposure mode.
    #[cfg(feature = "async")]
    async fn exposure_auto(&self) -> Result<(), Error>;

    /// Set manual exposure mode.
    #[cfg(not(feature = "async"))]
    fn exposure_manual(&mut self) -> Result<(), Error>;

    /// Set manual exposure mode.
    #[cfg(feature = "async")]
    async fn exposure_manual(&self) -> Result<(), Error>;

    /// Set iris level.
    #[cfg(not(feature = "async"))]
    fn set_iris(&mut self, level: crate::types::IrisLevel) -> Result<(), Error>;

    /// Set iris level.
    #[cfg(feature = "async")]
    async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error>;

    /// Set brightness level.
    #[cfg(not(feature = "async"))]
    fn set_brightness(&mut self, level: crate::types::BrightnessLevel) -> Result<(), Error>;

    /// Set brightness level.
    #[cfg(feature = "async")]
    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error>;

    /// Set backlight compensation.
    #[cfg(not(feature = "async"))]
    fn set_backlight(&mut self, enabled: bool) -> Result<(), Error>;

    /// Set backlight compensation.
    #[cfg(feature = "async")]
    async fn set_backlight(&self, enabled: bool) -> Result<(), Error>;

    /// Set gain value.
    #[cfg(not(feature = "async"))]
    fn set_gain(&mut self, gain: crate::types::Gain) -> Result<(), Error>;

    /// Set gain value.
    #[cfg(feature = "async")]
    async fn set_gain(&self, gain: crate::types::Gain) -> Result<(), Error>;

    /// Set gain limit.
    #[cfg(not(feature = "async"))]
    fn set_gain_limit(&mut self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set gain limit.
    #[cfg(feature = "async")]
    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set dynamic range level.
    #[cfg(not(feature = "async"))]
    fn set_dynamic_range(&mut self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set dynamic range level.
    #[cfg(feature = "async")]
    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set color temperature.
    #[cfg(not(feature = "async"))]
    fn set_color_temperature(&mut self, temp: crate::types::ColorTemperature) -> Result<(), Error>;

    /// Set color temperature.
    #[cfg(feature = "async")]
    async fn set_color_temperature(
        &self,
        temp: crate::types::ColorTemperature,
    ) -> Result<(), Error>;
}

// Blanket implementation for cameras with exposure support - blocking
#[cfg(not(feature = "async"))]
impl<P, T> ExposureMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + Exposure,
    T: crate::transport::blocking::BlockingTransport,
{
    #[dual_native_method]
    fn exposure_auto(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_AUTO);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn exposure_manual(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_MANUAL);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_iris(&mut self, level: crate::types::IrisLevel) -> Result<(), Error> {
        use crate::command::exposure::IrisCommand;

        let cmd = IrisCommand::SetAperture(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_brightness(&mut self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        use crate::command::exposure::BrightCommand;

        let cmd = BrightCommand::SetLevel(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_backlight(&mut self, enabled: bool) -> Result<(), Error> {
        use crate::command::image::BacklightCommand;

        let cmd = BacklightCommand { status: enabled };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_gain(&mut self, gain: crate::types::Gain) -> Result<(), Error> {
        use crate::command::gain::GainCommand;

        let cmd = GainCommand::SetValue(gain);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_gain_limit(&mut self, limit: crate::types::GainLimit) -> Result<(), Error> {
        use crate::command::gain::GainLimitCommand;

        let cmd = GainLimitCommand { limit };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_dynamic_range(&mut self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        use crate::command::exposure::DynamicRangeCommand;

        let cmd = DynamicRangeCommand::SetLevel(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_color_temperature(&mut self, temp: crate::types::ColorTemperature) -> Result<(), Error> {
        use crate::command::color::ColorTemperatureCommand;

        let cmd = ColorTemperatureCommand::SetTemperature(temp);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

// Blanket implementation for cameras with exposure support - async
#[cfg(feature = "async")]
impl<P, T> ExposureMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + Exposure,
    T: crate::transport::AsyncTransport,
{
    #[dual_native_method]
    fn exposure_auto(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_AUTO);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn exposure_manual(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_MANUAL);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_iris(&mut self, level: crate::types::IrisLevel) -> Result<(), Error> {
        use crate::command::exposure::IrisCommand;

        let cmd = IrisCommand::SetAperture(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_brightness(&mut self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        use crate::command::exposure::BrightCommand;

        let cmd = BrightCommand::SetLevel(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_backlight(&mut self, enabled: bool) -> Result<(), Error> {
        use crate::command::image::BacklightCommand;

        let cmd = BacklightCommand { status: enabled };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_gain(&mut self, gain: crate::types::Gain) -> Result<(), Error> {
        use crate::command::gain::GainCommand;

        let cmd = GainCommand::SetValue(gain);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_gain_limit(&mut self, limit: crate::types::GainLimit) -> Result<(), Error> {
        use crate::command::gain::GainLimitCommand;

        let cmd = GainLimitCommand { limit };
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_dynamic_range(&mut self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        use crate::command::exposure::DynamicRangeCommand;

        let cmd = DynamicRangeCommand::SetLevel(level);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
    fn set_color_temperature(&mut self, temp: crate::types::ColorTemperature) -> Result<(), Error> {
        use crate::command::color::ColorTemperatureCommand;

        let cmd = ColorTemperatureCommand::SetTemperature(temp);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}
