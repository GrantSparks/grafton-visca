//! Exposure methods for unified camera API.

use crate::Error;

/// Exposure operations for cameras.
///
/// This trait provides exposure control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait ExposureControl {
    /// Set exposure mode to any supported mode.
    #[cfg(feature = "async")]
    fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set exposure mode to any supported mode.
    #[cfg(not(feature = "async"))]
    fn set_exposure_mode(
        &mut self,
        mode: crate::command::exposure::ExposureMode,
    ) -> Result<(), Error>;

    /// Set auto exposure mode.
    #[cfg(feature = "async")]
    fn exposure_auto(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set auto exposure mode.
    #[cfg(not(feature = "async"))]
    fn exposure_auto(&mut self) -> Result<(), Error>;

    /// Set manual exposure mode.
    #[cfg(feature = "async")]
    fn exposure_manual(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set manual exposure mode.
    #[cfg(not(feature = "async"))]
    fn exposure_manual(&mut self) -> Result<(), Error>;

    /// Set shutter priority exposure mode.
    /// User controls shutter speed, camera adjusts other parameters.
    #[cfg(feature = "async")]
    fn exposure_shutter_priority(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set shutter priority exposure mode.
    /// User controls shutter speed, camera adjusts other parameters.
    #[cfg(not(feature = "async"))]
    fn exposure_shutter_priority(&mut self) -> Result<(), Error>;

    /// Set iris priority exposure mode.
    /// User controls iris/aperture, camera adjusts other parameters.
    #[cfg(feature = "async")]
    fn exposure_iris_priority(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set iris priority exposure mode.
    /// User controls iris/aperture, camera adjusts other parameters.
    #[cfg(not(feature = "async"))]
    fn exposure_iris_priority(&mut self) -> Result<(), Error>;

    /// Set brightness priority exposure mode.
    /// User controls brightness level, camera adjusts other parameters.
    #[cfg(feature = "async")]
    fn exposure_bright_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set brightness priority exposure mode.
    /// User controls brightness level, camera adjusts other parameters.
    #[cfg(not(feature = "async"))]
    fn exposure_bright_mode(&mut self) -> Result<(), Error>;

    /// Set iris level.
    #[cfg(feature = "async")]
    fn set_iris(
        &self,
        level: crate::types::IrisLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set iris level.
    #[cfg(not(feature = "async"))]
    fn set_iris(&mut self, level: crate::types::IrisLevel) -> Result<(), Error>;

    /// Reset iris to default.
    #[cfg(feature = "async")]
    fn reset_iris(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset iris to default.
    #[cfg(not(feature = "async"))]
    fn reset_iris(&mut self) -> Result<(), Error>;

    /// Increase iris (open aperture).
    #[cfg(feature = "async")]
    fn increase_iris(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Increase iris (open aperture).
    #[cfg(not(feature = "async"))]
    fn increase_iris(&mut self) -> Result<(), Error>;

    /// Decrease iris (close aperture).
    #[cfg(feature = "async")]
    fn decrease_iris(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Decrease iris (close aperture).
    #[cfg(not(feature = "async"))]
    fn decrease_iris(&mut self) -> Result<(), Error>;

    /// Set brightness level.
    #[cfg(feature = "async")]
    fn set_brightness(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set brightness level.
    #[cfg(not(feature = "async"))]
    fn set_brightness(&mut self, level: crate::types::BrightnessLevel) -> Result<(), Error>;

    /// Reset brightness to default.
    #[cfg(feature = "async")]
    fn reset_brightness(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset brightness to default.
    #[cfg(not(feature = "async"))]
    fn reset_brightness(&mut self) -> Result<(), Error>;

    /// Increase brightness.
    #[cfg(feature = "async")]
    fn increase_brightness(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Increase brightness.
    #[cfg(not(feature = "async"))]
    fn increase_brightness(&mut self) -> Result<(), Error>;

    /// Decrease brightness.
    #[cfg(feature = "async")]
    fn decrease_brightness(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Decrease brightness.
    #[cfg(not(feature = "async"))]
    fn decrease_brightness(&mut self) -> Result<(), Error>;

    /// Set backlight compensation.
    #[cfg(feature = "async")]
    fn set_backlight(
        &self,
        enabled: bool,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set backlight compensation.
    #[cfg(not(feature = "async"))]
    fn set_backlight(&mut self, enabled: bool) -> Result<(), Error>;

    /// Set gain value.
    #[cfg(feature = "async")]
    fn set_gain(
        &self,
        gain: crate::types::GainLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set gain value.
    #[cfg(not(feature = "async"))]
    fn set_gain(&mut self, gain: crate::types::GainLevel) -> Result<(), Error>;

    /// Reset gain to default.
    #[cfg(feature = "async")]
    fn reset_gain(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset gain to default.
    #[cfg(not(feature = "async"))]
    fn reset_gain(&mut self) -> Result<(), Error>;

    /// Increase gain by one step.
    #[cfg(feature = "async")]
    fn increase_gain(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Increase gain by one step.
    #[cfg(not(feature = "async"))]
    fn increase_gain(&mut self) -> Result<(), Error>;

    /// Decrease gain by one step.
    #[cfg(feature = "async")]
    fn decrease_gain(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Decrease gain by one step.
    #[cfg(not(feature = "async"))]
    fn decrease_gain(&mut self) -> Result<(), Error>;

    /// Set gain limit.
    #[cfg(feature = "async")]
    fn set_gain_limit(
        &self,
        limit: crate::types::GainLimit,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set gain limit.
    #[cfg(not(feature = "async"))]
    fn set_gain_limit(&mut self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set dynamic range level.
    #[cfg(feature = "async")]
    fn set_dynamic_range(
        &self,
        level: crate::types::DynamicRangeLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set dynamic range level.
    #[cfg(not(feature = "async"))]
    fn set_dynamic_range(&mut self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set color temperature.
    #[cfg(feature = "async")]
    fn set_color_temperature(
        &self,
        temp: crate::types::ColorTemp,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set color temperature.
    #[cfg(not(feature = "async"))]
    fn set_color_temperature(&mut self, temp: crate::types::ColorTemp) -> Result<(), Error>;

    /// Set shutter speed.
    #[cfg(feature = "async")]
    fn set_shutter_speed(
        &self,
        speed: crate::types::ShutterSpeed,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set shutter speed.
    #[cfg(not(feature = "async"))]
    fn set_shutter_speed(&mut self, speed: crate::types::ShutterSpeed) -> Result<(), Error>;

    /// Reset shutter speed to default.
    #[cfg(feature = "async")]
    fn reset_shutter_speed(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset shutter speed to default.
    #[cfg(not(feature = "async"))]
    fn reset_shutter_speed(&mut self) -> Result<(), Error>;

    /// Increase shutter speed (faster).
    #[cfg(feature = "async")]
    fn increase_shutter_speed(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Increase shutter speed (faster).
    #[cfg(not(feature = "async"))]
    fn increase_shutter_speed(&mut self) -> Result<(), Error>;

    /// Decrease shutter speed (slower).
    #[cfg(feature = "async")]
    fn decrease_shutter_speed(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Decrease shutter speed (slower).
    #[cfg(not(feature = "async"))]
    fn decrease_shutter_speed(&mut self) -> Result<(), Error>;

    /// Enable spotlight mode (Sony models).
    /// Enhances exposure for specific subjects.
    #[cfg(feature = "async")]
    fn enable_spotlight(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Enable spotlight mode (Sony models).
    /// Enhances exposure for specific subjects.
    #[cfg(not(feature = "async"))]
    fn enable_spotlight(&mut self) -> Result<(), Error>;

    /// Disable spotlight mode (Sony models).
    #[cfg(feature = "async")]
    fn disable_spotlight(&self)
        -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable spotlight mode (Sony models).
    #[cfg(not(feature = "async"))]
    fn disable_spotlight(&mut self) -> Result<(), Error>;

    /// Enable auto slow shutter mode.
    /// Automatically reduces shutter speed in low light conditions.
    /// Supported on Sony cameras and FR7, PtzOptics only via HTTP API.
    #[cfg(feature = "async")]
    fn enable_auto_slow_shutter(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Enable auto slow shutter mode.
    /// Automatically reduces shutter speed in low light conditions.
    /// Supported on Sony cameras and FR7, PtzOptics only via HTTP API.
    #[cfg(not(feature = "async"))]
    fn enable_auto_slow_shutter(&mut self) -> Result<(), Error>;

    /// Disable auto slow shutter mode.
    #[cfg(feature = "async")]
    fn disable_auto_slow_shutter(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable auto slow shutter mode.
    #[cfg(not(feature = "async"))]
    fn disable_auto_slow_shutter(&mut self) -> Result<(), Error>;

    /// Set brightness using direct mode.
    /// This is supported on Sony models but not on FR7.
    #[cfg(feature = "async")]
    fn set_brightness_direct(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set brightness using direct mode.
    /// This is supported on Sony models but not on FR7.
    #[cfg(not(feature = "async"))]
    fn set_brightness_direct(&mut self, level: crate::types::BrightnessLevel) -> Result<(), Error>;
}

// Keep the old trait names for backward compatibility during transition
/// Async exposure control trait (deprecated, use ExposureControl instead).
/// Blocking exposure control trait (deprecated, use ExposureControl instead).
// Async implementation for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> ExposureControl for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> Result<(), Error> {
        let cmd = crate::command::exposure::ExposureCommand { mode };
        self.send_command(&cmd).await?;
        Ok(())
    }
    async fn exposure_auto(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Auto)
            .await
    }

    async fn exposure_manual(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Manual)
            .await
    }

    async fn exposure_shutter_priority(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Shutter)
            .await
    }

    async fn exposure_iris_priority(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Iris)
            .await
    }

    async fn exposure_bright_mode(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Bright)
            .await
    }

    async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::SetAperture(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn reset_iris(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::Reset;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn increase_iris(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::Up;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn decrease_iris(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::Down;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::SetLevel(level);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn reset_brightness(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Reset;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn increase_brightness(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Up;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn decrease_brightness(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Down;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        let cmd = crate::command::image::BacklightCommand::new(enabled);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::SetValue(gain);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn reset_gain(&self) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::Reset;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn increase_gain(&self) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::Up;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn decrease_gain(&self) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::Down;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        let cmd = crate::command::gain::GainLimitCommand { limit };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        // DynamicRange command exists in exposure module
        let cmd = crate::command::exposure::DynamicRange { level };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error> {
        // First set white balance mode to ColorTemperature
        let wb_cmd = crate::command::white_balance::WhiteBalanceCommand {
            mode: crate::command::white_balance::WhiteBalanceMode::ColorTemperature,
        };
        self.send_command(&wb_cmd).await?;

        // Then set the actual temperature value
        let cmd = crate::command::color::ColorTemperature::SetTemperature(temp);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::SetSpeed(speed);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn reset_shutter_speed(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::Reset;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn increase_shutter_speed(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::Up;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn decrease_shutter_speed(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::Down;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn enable_spotlight(&self) -> Result<(), Error> {
        // Spotlight command not yet implemented
        Err(Error::Unsupported)
    }

    async fn disable_spotlight(&self) -> Result<(), Error> {
        // Spotlight command not yet implemented
        Err(Error::Unsupported)
    }

    async fn enable_auto_slow_shutter(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::AutoSlowShutter::On;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn disable_auto_slow_shutter(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::AutoSlowShutter::Off;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_brightness_direct(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Direct(level);
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> ExposureControl for crate::camera::BlockingCamera<P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn set_exposure_mode(
        &mut self,
        mode: crate::command::exposure::ExposureMode,
    ) -> Result<(), Error> {
        let cmd = crate::command::exposure::ExposureCommand { mode };
        self.send_command(&cmd)?;
        Ok(())
    }
    fn exposure_auto(&mut self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Auto)
    }

    fn exposure_manual(&mut self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Manual)
    }

    fn exposure_shutter_priority(&mut self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Shutter)
    }

    fn exposure_iris_priority(&mut self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Iris)
    }

    fn exposure_bright_mode(&mut self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Bright)
    }

    fn set_iris(&mut self, level: crate::types::IrisLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::SetAperture(level);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn reset_iris(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn increase_iris(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::Up;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn decrease_iris(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::Down;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_brightness(&mut self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::SetLevel(level);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn reset_brightness(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn increase_brightness(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Up;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn decrease_brightness(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Down;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_backlight(&mut self, enabled: bool) -> Result<(), Error> {
        let cmd = crate::command::image::BacklightCommand::new(enabled);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_gain(&mut self, gain: crate::types::GainLevel) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::SetValue(gain);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn reset_gain(&mut self) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn increase_gain(&mut self) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::Up;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn decrease_gain(&mut self) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::Down;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_gain_limit(&mut self, limit: crate::types::GainLimit) -> Result<(), Error> {
        let cmd = crate::command::gain::GainLimitCommand { limit };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_dynamic_range(&mut self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::DynamicRange { level };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_color_temperature(&mut self, temp: crate::types::ColorTemp) -> Result<(), Error> {
        // First set white balance mode to ColorTemperature
        let wb_cmd = crate::command::white_balance::WhiteBalanceCommand {
            mode: crate::command::white_balance::WhiteBalanceMode::ColorTemperature,
        };
        self.send_command(&wb_cmd)?;

        // Then set the actual temperature value
        let cmd = crate::command::color::ColorTemperature::SetTemperature(temp);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_shutter_speed(&mut self, speed: crate::types::ShutterSpeed) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::SetSpeed(speed);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn reset_shutter_speed(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn increase_shutter_speed(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::Up;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn decrease_shutter_speed(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::Down;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn enable_spotlight(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Spotlight::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn disable_spotlight(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Spotlight::Off;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn enable_auto_slow_shutter(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::AutoSlowShutter::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn disable_auto_slow_shutter(&mut self) -> Result<(), Error> {
        let cmd = crate::command::exposure::AutoSlowShutter::Off;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_brightness_direct(&mut self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Direct(level);
        self.send_command(&cmd)?;
        Ok(())
    }
}

/// Exposure compensation operations for cameras.
///
/// These methods are only available for cameras that support exposure compensation.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait ExposureCompensationControl {
    /// Enable exposure compensation.
    #[cfg(feature = "async")]
    fn enable_exposure_compensation(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Enable exposure compensation.
    #[cfg(not(feature = "async"))]
    fn enable_exposure_compensation(&mut self) -> Result<(), Error>;

    /// Disable exposure compensation.
    #[cfg(feature = "async")]
    fn disable_exposure_compensation(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable exposure compensation.
    #[cfg(not(feature = "async"))]
    fn disable_exposure_compensation(&mut self) -> Result<(), Error>;

    /// Reset exposure compensation to default.
    #[cfg(feature = "async")]
    fn reset_exposure_compensation(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset exposure compensation to default.
    #[cfg(not(feature = "async"))]
    fn reset_exposure_compensation(&mut self) -> Result<(), Error>;

    /// Increase exposure compensation by one step.
    #[cfg(feature = "async")]
    fn increase_exposure_compensation(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Increase exposure compensation by one step.
    #[cfg(not(feature = "async"))]
    fn increase_exposure_compensation(&mut self) -> Result<(), Error>;

    /// Decrease exposure compensation by one step.
    #[cfg(feature = "async")]
    fn decrease_exposure_compensation(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Decrease exposure compensation by one step.
    #[cfg(not(feature = "async"))]
    fn decrease_exposure_compensation(&mut self) -> Result<(), Error>;

    /// Set exposure compensation level (-7 to +7).
    #[cfg(feature = "async")]
    fn set_exposure_compensation_level(
        &self,
        level: i8,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set exposure compensation level (-7 to +7).
    #[cfg(not(feature = "async"))]
    fn set_exposure_compensation_level(&mut self, level: i8) -> Result<(), Error>;
}
