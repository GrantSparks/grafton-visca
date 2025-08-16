//! Exposure methods for cameras using the new GAT architecture.

use crate::Error;

/// Exposure operations (async).
#[cfg(feature = "async")]
pub trait ExposureOps: Sized {
    /// Set exposure mode to any supported mode.
    async fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> Result<(), Error>;

    /// Set auto exposure mode.
    async fn exposure_auto(&self) -> Result<(), Error>;

    /// Set manual exposure mode.
    async fn exposure_manual(&self) -> Result<(), Error>;

    /// Set shutter priority exposure mode.
    /// User controls shutter speed, camera adjusts other parameters.
    async fn exposure_shutter_priority(&self) -> Result<(), Error>;

    /// Set iris priority exposure mode.
    /// User controls iris/aperture, camera adjusts other parameters.
    async fn exposure_iris_priority(&self) -> Result<(), Error>;

    /// Set brightness priority exposure mode.
    /// User controls brightness level, camera adjusts other parameters.
    async fn exposure_bright_mode(&self) -> Result<(), Error>;

    /// Set iris level.
    async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error>;

    /// Reset iris to default.
    async fn reset_iris(&self) -> Result<(), Error>;

    /// Increase iris (open aperture).
    async fn increase_iris(&self) -> Result<(), Error>;

    /// Decrease iris (close aperture).
    async fn decrease_iris(&self) -> Result<(), Error>;

    /// Set brightness level.
    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error>;

    /// Reset brightness to default.
    async fn reset_brightness(&self) -> Result<(), Error>;

    /// Increase brightness.
    async fn increase_brightness(&self) -> Result<(), Error>;

    /// Decrease brightness.
    async fn decrease_brightness(&self) -> Result<(), Error>;

    /// Set backlight compensation.
    async fn set_backlight(&self, enabled: bool) -> Result<(), Error>;

    /// Set gain value.
    async fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error>;

    /// Reset gain to default.
    async fn reset_gain(&self) -> Result<(), Error>;

    /// Increase gain by one step.
    async fn increase_gain(&self) -> Result<(), Error>;

    /// Decrease gain by one step.
    async fn decrease_gain(&self) -> Result<(), Error>;

    /// Set gain limit.
    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set dynamic range level.
    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set color temperature.
    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error>;

    /// Set shutter speed.
    async fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error>;

    /// Reset shutter speed to default.
    async fn reset_shutter_speed(&self) -> Result<(), Error>;

    /// Increase shutter speed (faster).
    async fn increase_shutter_speed(&self) -> Result<(), Error>;

    /// Decrease shutter speed (slower).
    async fn decrease_shutter_speed(&self) -> Result<(), Error>;

    /// Enable spotlight mode (Sony models).
    /// Enhances exposure for specific subjects.
    async fn enable_spotlight(&self) -> Result<(), Error>;

    /// Disable spotlight mode (Sony models).
    async fn disable_spotlight(&self) -> Result<(), Error>;

    /// Enable auto slow shutter mode.
    /// Automatically reduces shutter speed in low light conditions.
    /// Supported on Sony cameras and FR7, PTZOptics only via HTTP API.
    async fn enable_auto_slow_shutter(&self) -> Result<(), Error>;

    /// Disable auto slow shutter mode.
    async fn disable_auto_slow_shutter(&self) -> Result<(), Error>;

    /// Set brightness using direct mode.
    /// This is supported on Sony models but not on FR7.
    async fn set_brightness_direct(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> ExposureOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
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

/// Exposure operations (blocking).
pub trait ExposureOpsBlocking: Sized {
    /// Set exposure mode to any supported mode.
    fn set_exposure_mode(&self, mode: crate::command::exposure::ExposureMode) -> Result<(), Error>;

    /// Set auto exposure mode.
    fn exposure_auto(&self) -> Result<(), Error>;

    /// Set manual exposure mode.
    fn exposure_manual(&self) -> Result<(), Error>;

    /// Set shutter priority exposure mode.
    /// User controls shutter speed, camera adjusts other parameters.
    fn exposure_shutter_priority(&self) -> Result<(), Error>;

    /// Set iris priority exposure mode.
    /// User controls iris/aperture, camera adjusts other parameters.
    fn exposure_iris_priority(&self) -> Result<(), Error>;

    /// Set brightness priority exposure mode.
    /// User controls brightness level, camera adjusts other parameters.
    fn exposure_bright_mode(&self) -> Result<(), Error>;

    /// Set iris level.
    fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error>;

    /// Reset iris to default.
    fn reset_iris(&self) -> Result<(), Error>;

    /// Increase iris (open aperture).
    fn increase_iris(&self) -> Result<(), Error>;

    /// Decrease iris (close aperture).
    fn decrease_iris(&self) -> Result<(), Error>;

    /// Set brightness level.
    fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error>;

    /// Reset brightness to default.
    fn reset_brightness(&self) -> Result<(), Error>;

    /// Increase brightness.
    fn increase_brightness(&self) -> Result<(), Error>;

    /// Decrease brightness.
    fn decrease_brightness(&self) -> Result<(), Error>;

    /// Set backlight compensation.
    fn set_backlight(&self, enabled: bool) -> Result<(), Error>;

    /// Set gain value.
    fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error>;

    /// Reset gain to default.
    fn reset_gain(&self) -> Result<(), Error>;

    /// Increase gain by one step.
    fn increase_gain(&self) -> Result<(), Error>;

    /// Decrease gain by one step.
    fn decrease_gain(&self) -> Result<(), Error>;

    /// Set gain limit.
    fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error>;

    /// Set dynamic range level.
    fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error>;

    /// Set color temperature.
    fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error>;

    /// Set shutter speed.
    fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error>;

    /// Reset shutter speed to default.
    fn reset_shutter_speed(&self) -> Result<(), Error>;

    /// Increase shutter speed (faster).
    fn increase_shutter_speed(&self) -> Result<(), Error>;

    /// Decrease shutter speed (slower).
    fn decrease_shutter_speed(&self) -> Result<(), Error>;

    /// Enable spotlight mode (Sony models).
    /// Enhances exposure for specific subjects.
    fn enable_spotlight(&self) -> Result<(), Error>;

    /// Disable spotlight mode (Sony models).
    fn disable_spotlight(&self) -> Result<(), Error>;

    /// Enable auto slow shutter mode.
    /// Automatically reduces shutter speed in low light conditions.
    /// Supported on Sony cameras and FR7, PTZOptics only via HTTP API.
    fn enable_auto_slow_shutter(&self) -> Result<(), Error>;

    /// Disable auto slow shutter mode.
    fn disable_auto_slow_shutter(&self) -> Result<(), Error>;

    /// Set brightness using direct mode.
    /// This is supported on Sony models but not on FR7.
    fn set_brightness_direct(&self, level: crate::types::BrightnessLevel) -> Result<(), Error>;
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> ExposureOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn set_exposure_mode(&self, mode: crate::command::exposure::ExposureMode) -> Result<(), Error> {
        let cmd = crate::command::exposure::ExposureCommand { mode };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn exposure_auto(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Auto)
    }

    fn exposure_manual(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Manual)
    }

    fn exposure_shutter_priority(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Shutter)
    }

    fn exposure_iris_priority(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Iris)
    }

    fn exposure_bright_mode(&self) -> Result<(), Error> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Bright)
    }

    fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::SetAperture(level);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn reset_iris(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn increase_iris(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::Up;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn decrease_iris(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Iris::Down;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::SetLevel(level);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn reset_brightness(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn increase_brightness(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Up;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn decrease_brightness(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Down;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        let cmd = crate::command::image::BacklightCommand::new(enabled);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::SetValue(gain);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn reset_gain(&self) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn increase_gain(&self) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::Up;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn decrease_gain(&self) -> Result<(), Error> {
        let cmd = crate::command::gain::Gain::Down;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        let cmd = crate::command::gain::GainLimitCommand::new(limit);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::DynamicRange { level };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error> {
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

    fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::SetSpeed(speed);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn reset_shutter_speed(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn increase_shutter_speed(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::Up;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn decrease_shutter_speed(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Shutter::Down;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn enable_spotlight(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Spotlight::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn disable_spotlight(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::Spotlight::Off;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn enable_auto_slow_shutter(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::AutoSlowShutter::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn disable_auto_slow_shutter(&self) -> Result<(), Error> {
        let cmd = crate::command::exposure::AutoSlowShutter::Off;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_brightness_direct(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        let cmd = crate::command::exposure::Bright::Direct(level);
        self.send_command(&cmd)?;
        Ok(())
    }
}

/// Exposure compensation operations (async).
///
/// These methods are only available for cameras that support exposure compensation.
#[cfg(feature = "async")]
pub trait ExposureCompensationOps: Sized {
    /// Enable exposure compensation.
    async fn enable_exposure_compensation(&self) -> Result<(), Error>;

    /// Disable exposure compensation.
    async fn disable_exposure_compensation(&self) -> Result<(), Error>;

    /// Reset exposure compensation to default.
    async fn reset_exposure_compensation(&self) -> Result<(), Error>;

    /// Increase exposure compensation by one step.
    async fn increase_exposure_compensation(&self) -> Result<(), Error>;

    /// Decrease exposure compensation by one step.
    async fn decrease_exposure_compensation(&self) -> Result<(), Error>;

    /// Set exposure compensation level (-7 to +7).
    async fn set_exposure_compensation_level(&self, level: i8) -> Result<(), Error>;
}

/// Exposure compensation operations (blocking).
///
/// These methods are only available for cameras that support exposure compensation.
pub trait ExposureCompensationOpsBlocking: Sized {
    /// Enable exposure compensation.
    fn enable_exposure_compensation(&self) -> Result<(), Error>;

    /// Disable exposure compensation.
    fn disable_exposure_compensation(&self) -> Result<(), Error>;

    /// Reset exposure compensation to default.
    fn reset_exposure_compensation(&self) -> Result<(), Error>;

    /// Increase exposure compensation by one step.
    fn increase_exposure_compensation(&self) -> Result<(), Error>;

    /// Decrease exposure compensation by one step.
    fn decrease_exposure_compensation(&self) -> Result<(), Error>;

    /// Set exposure compensation level (-7 to +7).
    fn set_exposure_compensation_level(&self, level: i8) -> Result<(), Error>;
}
