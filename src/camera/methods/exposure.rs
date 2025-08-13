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
impl<P, T> ExposureOps for crate::camera::Camera<crate::camera::AsyncMode, P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
{
    async fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> Result<(), Error> {
        self.set_exposure_mode(mode).await
    }

    async fn exposure_auto(&self) -> Result<(), Error> {
        self.exposure_auto().await
    }

    async fn exposure_manual(&self) -> Result<(), Error> {
        self.exposure_manual().await
    }

    async fn exposure_shutter_priority(&self) -> Result<(), Error> {
        self.exposure_shutter_priority().await
    }

    async fn exposure_iris_priority(&self) -> Result<(), Error> {
        self.exposure_iris_priority().await
    }

    async fn exposure_bright_mode(&self) -> Result<(), Error> {
        self.exposure_bright_mode().await
    }

    async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        self.set_iris(level).await
    }

    async fn reset_iris(&self) -> Result<(), Error> {
        self.reset_iris().await
    }

    async fn increase_iris(&self) -> Result<(), Error> {
        self.increase_iris().await
    }

    async fn decrease_iris(&self) -> Result<(), Error> {
        self.decrease_iris().await
    }

    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        self.set_brightness(level).await
    }

    async fn reset_brightness(&self) -> Result<(), Error> {
        self.reset_brightness().await
    }

    async fn increase_brightness(&self) -> Result<(), Error> {
        self.increase_brightness().await
    }

    async fn decrease_brightness(&self) -> Result<(), Error> {
        self.decrease_brightness().await
    }

    async fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        self.set_backlight(enabled).await
    }

    async fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
        self.set_gain(gain).await
    }

    async fn reset_gain(&self) -> Result<(), Error> {
        self.reset_gain().await
    }

    async fn increase_gain(&self) -> Result<(), Error> {
        self.increase_gain().await
    }

    async fn decrease_gain(&self) -> Result<(), Error> {
        self.decrease_gain().await
    }

    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        self.set_gain_limit(limit).await
    }

    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        // DynamicRange command not yet implemented
        self.set_dynamic_range(level).await
    }

    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error> {
        self.set_color_temperature(temp.value()).await
    }

    async fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error> {
        self.set_shutter_speed(speed).await
    }

    async fn reset_shutter_speed(&self) -> Result<(), Error> {
        self.reset_shutter_speed().await
    }

    async fn increase_shutter_speed(&self) -> Result<(), Error> {
        self.increase_shutter_speed().await
    }

    async fn decrease_shutter_speed(&self) -> Result<(), Error> {
        self.decrease_shutter_speed().await
    }

    async fn enable_spotlight(&self) -> Result<(), Error> {
        self.enable_spotlight().await
    }

    async fn disable_spotlight(&self) -> Result<(), Error> {
        self.disable_spotlight().await
    }

    async fn enable_auto_slow_shutter(&self) -> Result<(), Error> {
        self.enable_auto_slow_shutter().await
    }

    async fn disable_auto_slow_shutter(&self) -> Result<(), Error> {
        self.disable_auto_slow_shutter().await
    }

    async fn set_brightness_direct(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> Result<(), Error> {
        self.set_brightness_direct(level).await
    }
}

/// Exposure operations (blocking).
#[cfg(not(feature = "async"))]
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
#[cfg(not(feature = "async"))]
impl<P, T> ExposureOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn set_exposure_mode(&self, mode: crate::command::exposure::ExposureMode) -> Result<(), Error> {
        self.set_exposure_mode(mode)
    }

    fn exposure_auto(&self) -> Result<(), Error> {
        self.exposure_auto()
    }

    fn exposure_manual(&self) -> Result<(), Error> {
        self.exposure_manual()
    }

    fn exposure_shutter_priority(&self) -> Result<(), Error> {
        self.exposure_shutter_priority()
    }

    fn exposure_iris_priority(&self) -> Result<(), Error> {
        self.exposure_iris_priority()
    }

    fn exposure_bright_mode(&self) -> Result<(), Error> {
        self.exposure_bright_mode()
    }

    fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
        self.set_iris(level)
    }

    fn reset_iris(&self) -> Result<(), Error> {
        self.reset_iris()
    }

    fn increase_iris(&self) -> Result<(), Error> {
        self.increase_iris()
    }

    fn decrease_iris(&self) -> Result<(), Error> {
        self.decrease_iris()
    }

    fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        self.set_brightness(level)
    }

    fn reset_brightness(&self) -> Result<(), Error> {
        self.reset_brightness()
    }

    fn increase_brightness(&self) -> Result<(), Error> {
        self.increase_brightness()
    }

    fn decrease_brightness(&self) -> Result<(), Error> {
        self.decrease_brightness()
    }

    fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        self.set_backlight(enabled)
    }

    fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
        self.set_gain(gain)
    }

    fn reset_gain(&self) -> Result<(), Error> {
        self.reset_gain()
    }

    fn increase_gain(&self) -> Result<(), Error> {
        self.increase_gain()
    }

    fn decrease_gain(&self) -> Result<(), Error> {
        self.decrease_gain()
    }

    fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
        self.set_gain_limit(limit)
    }

    fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
        self.set_dynamic_range(level)
    }

    fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error> {
        self.set_color_temperature(temp.into())
    }

    fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error> {
        self.set_shutter_speed(speed)
    }

    fn reset_shutter_speed(&self) -> Result<(), Error> {
        self.reset_shutter_speed()
    }

    fn increase_shutter_speed(&self) -> Result<(), Error> {
        self.increase_shutter_speed()
    }

    fn decrease_shutter_speed(&self) -> Result<(), Error> {
        self.decrease_shutter_speed()
    }

    fn enable_spotlight(&self) -> Result<(), Error> {
        self.enable_spotlight()
    }

    fn disable_spotlight(&self) -> Result<(), Error> {
        self.disable_spotlight()
    }

    fn enable_auto_slow_shutter(&self) -> Result<(), Error> {
        self.enable_auto_slow_shutter()
    }

    fn disable_auto_slow_shutter(&self) -> Result<(), Error> {
        self.disable_auto_slow_shutter()
    }

    fn set_brightness_direct(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
        self.set_brightness_direct(level)
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
#[cfg(not(feature = "async"))]
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

// Async implementation
// #[cfg(feature = "async")] - temporarily disabled
// // impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
// //     ExposureOps for crate::camera::generic::Camera<P, T>
// // where
// //     T::Error: Into<Error> + Send,
// //     for<'a> T::SendFut<'a>: Send,
// //     for<'a> T::RecvFut<'a>: Send,
// // {
// //     async fn set_exposure_mode(
// //         &self,
// //         mode: crate::command::exposure::ExposureMode,
// //     ) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureCommand;
// //
// //         let command = ExposureCommand { mode };
// //         self.send_command(&command).await?;
// //         Ok(())
// //     }
// //
// //     async fn exposure_auto(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         ExposureOps::set_exposure_mode(self, ExposureMode::Auto).await
// //     }
// //
// //     async fn exposure_manual(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         ExposureOps::set_exposure_mode(self, ExposureMode::Manual).await
// //     }
// //
// //     async fn exposure_shutter_priority(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         ExposureOps::set_exposure_mode(self, ExposureMode::Shutter).await
// //     }
// //
// //     async fn exposure_iris_priority(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         ExposureOps::set_exposure_mode(self, ExposureMode::Iris).await
// //     }
// //
// //     async fn exposure_bright_mode(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         ExposureOps::set_exposure_mode(self, ExposureMode::Bright).await
// //     }
// //
// //     async fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
// //         use crate::command::exposure::Iris;
// //
// //         let command = Iris::SetAperture(level);
// //         self.send_command(&command).await?;
// //         Ok(())
// //     }
// //
// //     async fn reset_iris(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Iris;
// //
// //         self.send_command(&Iris::Reset).await?;
// //         Ok(())
// //     }
// //
// //     async fn increase_iris(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Iris;
// //
// //         self.send_command(&Iris::Up).await?;
// //         Ok(())
// //     }
// //
// //     async fn decrease_iris(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Iris;
// //
// //         self.send_command(&Iris::Down).await?;
// //         Ok(())
// //     }
// //
// //     async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         let command = Bright::SetLevel(level);
// //         self.send_command(&command).await?;
// //         Ok(())
// //     }
// //
// //     async fn reset_brightness(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         self.send_command(&Bright::Reset).await?;
// //         Ok(())
// //     }
// //
// //     async fn increase_brightness(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         self.send_command(&Bright::Up).await?;
// //         Ok(())
// //     }
// //
// //     async fn decrease_brightness(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         self.send_command(&Bright::Down).await?;
// //         Ok(())
// //     }
// //
// //     async fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
// //         use crate::command::image::BacklightCommand;
// //
// //         let command = BacklightCommand::new(enabled);
// //         self.send_command(&command).await?;
// //         Ok(())
// //     }
// //
// //     async fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
// //         use crate::command::gain::Gain;
// //
// //         let command = Gain::SetValue(gain);
// //         self.send_command(&command).await?;
// //         Ok(())
// //     }
// //
// //     async fn reset_gain(&self) -> Result<(), Error> {
// //         use crate::command::gain::Gain;
// //
// //         self.send_command(&Gain::Reset).await?;
// //         Ok(())
// //     }
// //
// //     async fn increase_gain(&self) -> Result<(), Error> {
// //         use crate::command::gain::Gain;
// //
// //         self.send_command(&Gain::Up).await?;
// //         Ok(())
// //     }
// //
// //     async fn decrease_gain(&self) -> Result<(), Error> {
// //         use crate::command::gain::Gain;
// //
// //         self.send_command(&Gain::Down).await?;
// //         Ok(())
// //     }
// //
// //     async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
// //         use crate::command::gain::GainLimitCommand;
// //
// //         let command = GainLimitCommand::new(limit);
// //         self.send_command(&command).await?;
// //         Ok(())
// //     }
// //
// //     async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
// //         use crate::command::exposure::DynamicRange;
// //
// //         let command = DynamicRange::new(level);
// //         self.send_command(&command).await?;
// //         Ok(())
// //     }
// //
// //     async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error> {
// //         use crate::command::color::ColorTemperature;
// //
// //         let command = ColorTemperature::SetTemperature(temp);
// //         self.send_command(&command).await?;
// //         Ok(())
// //     }
// //
// //     async fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error> {
// //         use crate::command::exposure::Shutter;
// //
// //         self.send_command(&Shutter::SetSpeed(speed)).await?;
// //         Ok(())
// //     }
// //
// //     async fn reset_shutter_speed(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Shutter;
// //
// //         self.send_command(&Shutter::Reset).await?;
// //         Ok(())
// //     }
// //
// //     async fn increase_shutter_speed(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Shutter;
// //
// //         self.send_command(&Shutter::Up).await?;
// //         Ok(())
// //     }
// //
// //     async fn decrease_shutter_speed(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Shutter;
// //
// //         self.send_command(&Shutter::Down).await?;
// //         Ok(())
// //     }
// //
// //     async fn enable_spotlight(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Spotlight;
// //
// //         self.send_command(&Spotlight::On).await?;
// //         Ok(())
// //     }
// //
// //     async fn disable_spotlight(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Spotlight;
// //
// //         self.send_command(&Spotlight::Off).await?;
// //         Ok(())
// //     }
// //
// //     async fn enable_auto_slow_shutter(&self) -> Result<(), Error> {
// //         use crate::command::exposure::AutoSlowShutter;
// //
// //         self.send_command(&AutoSlowShutter::On).await?;
// //         Ok(())
// //     }
// //
// //     async fn disable_auto_slow_shutter(&self) -> Result<(), Error> {
// //         use crate::command::exposure::AutoSlowShutter;
// //
// //         self.send_command(&AutoSlowShutter::Off).await?;
// //         Ok(())
// //     }
// //
// //     async fn set_brightness_direct(
// //         &self,
// //         level: crate::types::BrightnessLevel,
// //     ) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         let command = Bright::Direct(level);
// //         self.send_command(&command).await?;
// //         Ok(())
// //     }
// // }
//
// // Blocking implementation
// #[cfg(not(feature = "async"))]
// // impl<
// //         P: crate::capabilities::Profile,
// //         T: crate::transport::Transport
// //             + Send
// //             + Sync
// //             + 'static
// //             + crate::transport::core::BlockingTransport,
// //     > ExposureOpsBlocking for crate::camera::generic::Camera<P, T>
// // where
// //     T::Error: Into<Error> + Send,
// //     for<'a> T::SendFut<'a>: Send,
// //     for<'a> T::RecvFut<'a>: Send,
// // {
// //     fn set_exposure_mode(&self, mode: crate::command::exposure::ExposureMode) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureCommand;
// //
// //         let command = ExposureCommand { mode };
// //         self.send_command_blocking(&command)?;
// //         Ok(())
// //     }
// //
// //     fn exposure_auto(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         Self::set_exposure_mode(self, ExposureMode::Auto)
// //     }
// //
// //     fn exposure_manual(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         Self::set_exposure_mode(self, ExposureMode::Manual)
// //     }
// //
// //     fn exposure_shutter_priority(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         Self::set_exposure_mode(self, ExposureMode::Shutter)
// //     }
// //
// //     fn exposure_iris_priority(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         Self::set_exposure_mode(self, ExposureMode::Iris)
// //     }
// //
// //     fn exposure_bright_mode(&self) -> Result<(), Error> {
// //         use crate::command::exposure::ExposureMode;
// //
// //         Self::set_exposure_mode(self, ExposureMode::Bright)
// //     }
// //
// //     fn set_iris(&self, level: crate::types::IrisLevel) -> Result<(), Error> {
// //         use crate::command::exposure::Iris;
// //
// //         let command = Iris::SetAperture(level);
// //         self.send_command_blocking(&command)?;
// //         Ok(())
// //     }
// //
// //     fn reset_iris(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Iris;
// //
// //         self.send_command_blocking(&Iris::Reset)?;
// //         Ok(())
// //     }
// //
// //     fn increase_iris(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Iris;
// //
// //         self.send_command_blocking(&Iris::Up)?;
// //         Ok(())
// //     }
// //
// //     fn decrease_iris(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Iris;
// //
// //         self.send_command_blocking(&Iris::Down)?;
// //         Ok(())
// //     }
// //
// //     fn set_brightness(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         let command = Bright::SetLevel(level);
// //         self.send_command_blocking(&command)?;
// //         Ok(())
// //     }
// //
// //     fn reset_brightness(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         self.send_command_blocking(&Bright::Reset)?;
// //         Ok(())
// //     }
// //
// //     fn increase_brightness(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         self.send_command_blocking(&Bright::Up)?;
// //         Ok(())
// //     }
// //
// //     fn decrease_brightness(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         self.send_command_blocking(&Bright::Down)?;
// //         Ok(())
// //     }
// //
// //     fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
// //         use crate::command::image::BacklightCommand;
// //
// //         let command = BacklightCommand::new(enabled);
// //         self.send_command_blocking(&command)?;
// //         Ok(())
// //     }
// //
// //     fn set_gain(&self, gain: crate::types::GainLevel) -> Result<(), Error> {
// //         use crate::command::gain::Gain;
// //
// //         let command = Gain::SetValue(gain);
// //         self.send_command_blocking(&command)?;
// //         Ok(())
// //     }
// //
// //     fn reset_gain(&self) -> Result<(), Error> {
// //         use crate::command::gain::Gain;
// //
// //         self.send_command_blocking(&Gain::Reset)?;
// //         Ok(())
// //     }
// //
// //     fn increase_gain(&self) -> Result<(), Error> {
// //         use crate::command::gain::Gain;
// //
// //         self.send_command_blocking(&Gain::Up)?;
// //         Ok(())
// //     }
// //
// //     fn decrease_gain(&self) -> Result<(), Error> {
// //         use crate::command::gain::Gain;
// //
// //         self.send_command_blocking(&Gain::Down)?;
// //         Ok(())
// //     }
// //
// //     fn set_gain_limit(&self, limit: crate::types::GainLimit) -> Result<(), Error> {
// //         use crate::command::gain::GainLimitCommand;
// //
// //         let command = GainLimitCommand::new(limit);
// //         self.send_command_blocking(&command)?;
// //         Ok(())
// //     }
// //
// //     fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> Result<(), Error> {
// //         use crate::command::exposure::DynamicRange;
// //
// //         let command = DynamicRange::new(level);
// //         self.send_command_blocking(&command)?;
// //         Ok(())
// //     }
// //
// //     fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> Result<(), Error> {
// //         use crate::command::color::ColorTemperature;
// //
// //         let command = ColorTemperature::SetTemperature(temp);
// //         self.send_command_blocking(&command)?;
// //         Ok(())
// //     }
// //
// //     fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> Result<(), Error> {
// //         use crate::command::exposure::Shutter;
// //
// //         self.send_command_blocking(&Shutter::SetSpeed(speed))?;
// //         Ok(())
// //     }
// //
// //     fn reset_shutter_speed(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Shutter;
// //
// //         self.send_command_blocking(&Shutter::Reset)?;
// //         Ok(())
// //     }
// //
// //     fn increase_shutter_speed(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Shutter;
// //
// //         self.send_command_blocking(&Shutter::Up)?;
// //         Ok(())
// //     }
// //
// //     fn decrease_shutter_speed(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Shutter;
// //
// //         self.send_command_blocking(&Shutter::Down)?;
// //         Ok(())
// //     }
// //
// //     fn enable_spotlight(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Spotlight;
// //
// //         self.send_command_blocking(&Spotlight::On)?;
// //         Ok(())
// //     }
// //
// //     fn disable_spotlight(&self) -> Result<(), Error> {
// //         use crate::command::exposure::Spotlight;
// //
// //         self.send_command_blocking(&Spotlight::Off)?;
// //         Ok(())
// //     }
// //
// //     fn enable_auto_slow_shutter(&self) -> Result<(), Error> {
// //         use crate::command::exposure::AutoSlowShutter;
// //
// //         self.send_command_blocking(&AutoSlowShutter::On)?;
// //         Ok(())
// //     }
// //
// //     fn disable_auto_slow_shutter(&self) -> Result<(), Error> {
// //         use crate::command::exposure::AutoSlowShutter;
// //
// //         self.send_command_blocking(&AutoSlowShutter::Off)?;
// //         Ok(())
// //     }
// //
// //     fn set_brightness_direct(&self, level: crate::types::BrightnessLevel) -> Result<(), Error> {
// //         use crate::command::exposure::Bright;
// //
// //         let command = Bright::Direct(level);
// //         self.send_command_blocking(&command)?;
// //         Ok(())
// //     }
// // }
//
// // Async implementation for exposure compensation - requires HasExposureCompensation marker trait
// #[cfg(feature = "async")]
// impl<P, T> ExposureCompensationOps for crate::camera::generic::Camera<P, T>
// where
//     P: crate::capabilities::Profile
//         + crate::capabilities::Exposure
//         + crate::capabilities::HasExposureCompensation,
//     T: crate::transport::Transport + Send + Sync + 'static,
//     T::Error: Into<Error> + Send,
//     for<'a> T::SendFut<'a>: Send,
//     for<'a> T::RecvFut<'a>: Send,
// {
//     async fn enable_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command(&ExposureCompensation::On).await?;
//         Ok(())
//     }
//
//     async fn disable_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command(&ExposureCompensation::Off).await?;
//         Ok(())
//     }
//
//     async fn reset_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command(&ExposureCompensation::Reset).await?;
//         Ok(())
//     }
//
//     async fn increase_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command(&ExposureCompensation::Up).await?;
//         Ok(())
//     }
//
//     async fn decrease_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command(&ExposureCompensation::Down).await?;
//         Ok(())
//     }
//
//     async fn set_exposure_compensation_level(&self, level: i8) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//         use crate::types::ExposureCompensationLevel;
//
//         let level = ExposureCompensationLevel::new(level)?;
//         self.send_command(&ExposureCompensation::SetLevel(level))
//             .await?;
//         Ok(())
//     }
// }

// Blocking implementation for exposure compensation - requires HasExposureCompensation marker trait
// Temporarily disabled during refactor
// #[cfg(not(feature = "async"))]
// impl<P, T> ExposureCompensationOpsBlocking for crate::camera::generic::Camera<P, T>
// where
//     P: crate::capabilities::Profile
//         + crate::capabilities::Exposure
//         + crate::capabilities::HasExposureCompensation,
//     T: crate::transport::Transport
//         + Send
//         + Sync
//         + 'static
//         + crate::transport::core::BlockingTransport,
//     T::Error: Into<Error> + Send,
//     for<'a> T::SendFut<'a>: Send,
//     for<'a> T::RecvFut<'a>: Send,
// {
//     fn enable_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command_blocking(&ExposureCompensation::On)?;
//         Ok(())
//     }
//
//     fn disable_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command_blocking(&ExposureCompensation::Off)?;
//         Ok(())
//     }
//
//     fn reset_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command_blocking(&ExposureCompensation::Reset)?;
//         Ok(())
//     }
//
//     fn increase_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command_blocking(&ExposureCompensation::Up)?;
//         Ok(())
//     }
//
//     fn decrease_exposure_compensation(&self) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//
//         self.send_command_blocking(&ExposureCompensation::Down)?;
//         Ok(())
//     }
//
//     fn set_exposure_compensation_level(&self, level: i8) -> Result<(), Error> {
//         use crate::command::exposure::ExposureCompensation;
//         use crate::types::ExposureCompensationLevel;
//
//         let level = ExposureCompensationLevel::new(level)?;
//         self.send_command_blocking(&ExposureCompensation::SetLevel(level))?;
//         Ok(())
//     }
// }
