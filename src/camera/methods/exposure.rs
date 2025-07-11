//! Exposure methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{Exposure, ProfileMetadata},
    command::{
        const_encoding::{commands, CommandBuilder},
        Command, Response, ResponseType,
    },
    transport::gat_transport::Transport,
    Error,
};
use core::future::Future;

/// Exposure auto command.
struct ExposureAutoCommand([u8; 6]);

impl ExposureAutoCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_AUTO);
        Self(cmd.build())
    }
}

impl Command for ExposureAutoCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Exposure manual command.
struct ExposureManualCommand([u8; 6]);

impl ExposureManualCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::EXPOSURE_MANUAL);
        Self(cmd.build())
    }
}

impl Command for ExposureManualCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Extension trait for CameraCore - provides future-returning methods.
pub trait ExposureCoreExt<P, T>
where
    P: ProfileMetadata + Exposure,
    T: Transport,
{
    /// Set auto exposure mode - returns a future.
    fn exposure_auto(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set manual exposure mode - returns a future.
    fn exposure_manual(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set iris level - returns a future.
    fn set_iris(
        &self,
        level: crate::types::IrisLevel,
    ) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set brightness level - returns a future.
    fn set_brightness(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set backlight compensation - returns a future.
    fn set_backlight(&self, enabled: bool) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set gain value - returns a future.
    fn set_gain(&self, gain: crate::types::Gain) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set gain limit - returns a future.
    fn set_gain_limit(
        &self,
        limit: crate::types::GainLimit,
    ) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set dynamic range level - returns a future.
    fn set_dynamic_range(
        &self,
        level: crate::types::DynamicRangeLevel,
    ) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set color temperature - returns a future.
    fn set_color_temperature(
        &self,
        temp: crate::types::ColorTemperature,
    ) -> impl Future<Output = Result<(), Error>> + '_;
}

impl<P, T> ExposureCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata + Exposure,
    T: Transport,
{
    fn exposure_auto(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = ExposureAutoCommand::new();
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn exposure_manual(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = ExposureManualCommand::new();
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_iris(
        &self,
        level: crate::types::IrisLevel,
    ) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            use crate::command::exposure::IrisCommand;

            let command = IrisCommand::SetAperture(level);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_brightness(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            use crate::command::exposure::BrightCommand;

            let command = BrightCommand::SetLevel(level);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_backlight(&self, enabled: bool) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            use crate::command::image::BacklightCommand;

            let command = BacklightCommand { status: enabled };
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_gain(&self, gain: crate::types::Gain) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            use crate::command::gain::GainCommand;

            let command = GainCommand::SetValue(gain);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_gain_limit(
        &self,
        limit: crate::types::GainLimit,
    ) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            use crate::command::gain::GainLimitCommand;

            let command = GainLimitCommand { limit };
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_dynamic_range(
        &self,
        level: crate::types::DynamicRangeLevel,
    ) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            use crate::command::exposure::DynamicRangeCommand;

            let command = DynamicRangeCommand::SetLevel(level);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_color_temperature(
        &self,
        temp: crate::types::ColorTemperature,
    ) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            use crate::command::color::ColorTemperatureCommand;

            let command = ColorTemperatureCommand::SetTemperature(temp);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for async Camera facade.
#[allow(async_fn_in_trait)]
pub trait ExposureAsyncExt<P, T>
where
    P: ProfileMetadata + Exposure,
    T: Transport,
{
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
    async fn set_gain(&self, gain: crate::types::Gain) -> Result<(), Error>;

    /// Set gain limit.
    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set dynamic range level.
    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set color temperature.
    async fn set_color_temperature(
        &self,
        temp: crate::types::ColorTemperature,
    ) -> Result<(), Error>;
}

impl<P, T> ExposureAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + Exposure,
    T: Transport,
{
    async fn exposure_auto(&self) -> Result<(), Error> {
        self.core().exposure_auto().await
    }

    async fn exposure_manual(&self) -> Result<(), Error> {
        self.core().exposure_manual().await
    }

    async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        self.core().set_iris(level).await
    }

    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        self.core().set_brightness(level).await
    }

    async fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        self.core().set_backlight(enabled).await
    }

    async fn set_gain(&self, gain: crate::types::Gain) -> Result<(), Error> {
        self.core().set_gain(gain).await
    }

    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        self.core().set_gain_limit(limit).await
    }

    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        self.core().set_dynamic_range(level).await
    }

    async fn set_color_temperature(
        &self,
        temp: crate::types::ColorTemperature,
    ) -> Result<(), Error> {
        self.core().set_color_temperature(temp).await
    }
}

/// Extension trait for blocking Camera facade.
pub trait ExposureBlockingExt<P, T>
where
    P: ProfileMetadata + Exposure,
    T: Transport,
{
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
    fn set_gain(&self, gain: crate::types::Gain) -> Result<(), Error>;

    /// Set gain limit.
    fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set dynamic range level.
    fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set color temperature.
    fn set_color_temperature(&self, temp: crate::types::ColorTemperature) -> Result<(), Error>;
}

impl<P, T> ExposureBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata + Exposure,
    T: Transport,
{
    fn exposure_auto(&self) -> Result<(), Error> {
        block_on(self.core().exposure_auto())
    }

    fn exposure_manual(&self) -> Result<(), Error> {
        block_on(self.core().exposure_manual())
    }

    fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        block_on(self.core().set_iris(level))
    }

    fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        block_on(self.core().set_brightness(level))
    }

    fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        block_on(self.core().set_backlight(enabled))
    }

    fn set_gain(&self, gain: crate::types::Gain) -> Result<(), Error> {
        block_on(self.core().set_gain(gain))
    }

    fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        block_on(self.core().set_gain_limit(limit))
    }

    fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        block_on(self.core().set_dynamic_range(level))
    }

    fn set_color_temperature(&self, temp: crate::types::ColorTemperature) -> Result<(), Error> {
        block_on(self.core().set_color_temperature(temp))
    }
}
