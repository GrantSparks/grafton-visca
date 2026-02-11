//! Exposure control implementation for PTZ cameras.
//!
//! This module provides comprehensive exposure control functionality including:
//! - Exposure mode selection (auto, manual, shutter priority, iris priority, brightness)
//! - Iris (aperture) control for depth of field adjustment
//! - Brightness and gain control for image luminance
//! - Shutter speed control for motion blur and exposure time
//! - Backlight compensation for challenging lighting conditions
//! - Dynamic range adjustment for high contrast scenes
//! - Spotlight mode for focused lighting scenarios
//! - Auto slow shutter for low light conditions
//! - Exposure compensation for fine-tuning exposure levels
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{camera::ViscaClient, mode::Mode, Error};

/// Exposure operations for PTZ cameras.
///
/// This trait provides comprehensive exposure control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Exposure Modes
///
/// - **Auto**: Camera automatically adjusts all exposure parameters
/// - **Manual**: User controls iris, shutter speed, and gain manually
/// - **Shutter Priority**: User sets shutter speed, camera adjusts iris
/// - **Iris Priority**: User sets iris, camera adjusts shutter speed
/// - **Brightness**: Camera maintains consistent brightness level
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.exposure_auto()?;  // Enable auto exposure
/// camera.exposure_manual()?;  // Switch to manual mode
/// camera.set_iris(IrisLevel::new(8)?)?;  // Set specific iris level
/// camera.set_gain(GainLevel::new(12)?)?;  // Adjust gain
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.exposure_auto().await?;  // Enable auto exposure
/// camera.exposure_manual().await?;  // Switch to manual mode
/// camera.set_iris(IrisLevel::new(8)?).await?;  // Set specific iris level
/// camera.set_gain(GainLevel::new(12)?).await?;  // Adjust gain
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait ExposureControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set exposure mode to any supported mode.
    ///
    /// Allows setting any of the supported exposure modes directly.
    ///
    /// # Parameters
    /// - `mode`: The exposure mode to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set auto exposure mode.
    ///
    /// In auto exposure mode, the camera automatically adjusts iris, shutter speed,
    /// and gain to maintain optimal exposure for the scene.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn exposure_auto(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set manual exposure mode.
    ///
    /// In manual exposure mode, all exposure parameters must be set manually.
    /// The camera will not automatically adjust iris, shutter speed, or gain.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn exposure_manual(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set shutter priority exposure mode.
    ///
    /// In shutter priority mode, you set the shutter speed and the camera
    /// automatically adjusts iris to maintain proper exposure.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn exposure_shutter_priority(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set iris priority exposure mode.
    ///
    /// In iris priority mode, you set the iris (aperture) and the camera
    /// automatically adjusts shutter speed to maintain proper exposure.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn exposure_iris_priority(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set brightness priority exposure mode.
    ///
    /// In brightness priority mode, the camera maintains a consistent brightness
    /// level by automatically adjusting exposure parameters.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn exposure_bright_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set iris level.
    ///
    /// Sets the iris (aperture) to a specific level. Lower values = more closed aperture
    /// (smaller opening, greater depth of field). Higher values = more open aperture
    /// (larger opening, shallower depth of field).
    ///
    /// # Parameters
    /// - `level`: The iris level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_iris(
        &self,
        level: crate::types::IrisLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset iris to default.
    ///
    /// Resets the iris to the camera's default level.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn reset_iris(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase iris (open aperture).
    ///
    /// Opens the aperture by one step, allowing more light in and reducing depth of field.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn increase_iris(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease iris (close aperture).
    ///
    /// Closes the aperture by one step, allowing less light in and increasing depth of field.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn decrease_iris(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set brightness level.
    ///
    /// Sets the overall brightness level of the image output.
    ///
    /// # Parameters
    /// - `level`: The brightness level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_brightness(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset brightness to default.
    ///
    /// Resets the brightness to the camera's default level.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn reset_brightness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase brightness.
    ///
    /// Increases the brightness level by one step.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn increase_brightness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease brightness.
    ///
    /// Decreases the brightness level by one step.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn decrease_brightness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set backlight compensation.
    ///
    /// Enables or disables backlight compensation to improve visibility when
    /// the subject is backlit (strong light source behind the subject).
    ///
    /// # Parameters
    /// - `enabled`: True to enable backlight compensation, false to disable
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_backlight(&self, enabled: bool) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set gain value.
    ///
    /// Sets the sensor gain level. Higher gain values increase image brightness
    /// but also increase noise, especially in low light conditions.
    ///
    /// # Parameters
    /// - `gain`: The gain level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_gain(
        &self,
        gain: crate::types::GainLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset gain to default.
    ///
    /// Resets the gain to the camera's default level.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn reset_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase gain by one step.
    ///
    /// Increases the sensor gain by one step, making the image brighter
    /// but potentially adding more noise.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn increase_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease gain by one step.
    ///
    /// Decreases the sensor gain by one step, making the image darker
    /// but reducing noise.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn decrease_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set gain limit.
    ///
    /// Sets the maximum gain level that auto exposure can use.
    /// This helps control noise in low light conditions.
    ///
    /// # Parameters
    /// - `limit`: The maximum gain limit to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_gain_limit(
        &self,
        limit: crate::types::GainLimit,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set dynamic range level.
    ///
    /// Adjusts the dynamic range processing to handle high contrast scenes.
    /// Higher levels provide better detail in both shadows and highlights.
    ///
    /// # Parameters
    /// - `level`: The dynamic range level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_dynamic_range(
        &self,
        level: crate::types::DynamicRangeLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set color temperature value.
    ///
    /// Sets the specific color temperature value when white balance is in color temperature mode.
    ///
    /// **Important:** The camera's white balance mode must first be set to `ColorTemperature` mode
    /// using [`WhiteBalanceControl::white_balance_color_temperature`] before calling this method.
    ///
    /// # Usage Pattern
    ///
    /// ```ignore
    /// // 1. First, switch to color temperature mode
    /// camera.white_balance_color_temperature()?;  // or .await? for async
    ///
    /// // 2. Then set the specific temperature value
    /// let temp = ColorTemp::from_kelvin(5600)?;  // Daylight color temp
    /// camera.set_color_temperature(temp)?;  // or .await? for async
    /// ```
    ///
    /// # Parameters
    /// - `temp`: The color temperature to set (2500K-8000K range)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response, or if the
    /// camera is not in color temperature white balance mode.
    ///
    /// [`WhiteBalanceControl::white_balance_color_temperature`]: crate::camera::controls::white_balance::WhiteBalanceControl::white_balance_color_temperature
    fn set_color_temperature(
        &self,
        temp: crate::types::ColorTemp,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set shutter speed.
    ///
    /// Sets the shutter speed to control motion blur and exposure time.
    /// Faster speeds freeze motion, slower speeds allow motion blur.
    ///
    /// # Parameters
    /// - `speed`: The shutter speed to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_shutter_speed(
        &self,
        speed: crate::types::ShutterSpeed,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset shutter speed to default.
    ///
    /// Resets the shutter speed to the camera's default setting.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn reset_shutter_speed(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase shutter speed (faster).
    ///
    /// Makes the shutter speed faster by one step, reducing motion blur
    /// and exposure time.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn increase_shutter_speed(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease shutter speed (slower).
    ///
    /// Makes the shutter speed slower by one step, increasing motion blur
    /// and exposure time.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn decrease_shutter_speed(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable spotlight mode (Sony models).
    ///
    /// Enables spotlight mode which optimizes exposure for scenes with
    /// a bright spotlight or focused lighting.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn enable_spotlight(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable spotlight mode (Sony models).
    ///
    /// Disables spotlight mode and returns to normal exposure behavior.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_spotlight(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable auto slow shutter mode.
    ///
    /// Enables automatic slow shutter mode which allows the camera to use
    /// slower shutter speeds in low light conditions for better exposure.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn enable_auto_slow_shutter(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable auto slow shutter mode.
    ///
    /// Disables automatic slow shutter mode, limiting the camera to
    /// normal shutter speed ranges.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_auto_slow_shutter(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set brightness using direct mode.
    ///
    /// Sets brightness level using direct command mode rather than
    /// incremental adjustments.
    ///
    /// # Parameters
    /// - `level`: The brightness level to set directly
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_brightness_direct(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set anti-flicker mode.
    ///
    /// Controls the camera's flicker reduction to match the local AC power frequency.
    /// Using the wrong setting can cause visible banding/flickering in the image.
    ///
    /// **Vendor-Specific**: PTZOptics cameras only.
    ///
    /// # Parameters
    /// - `mode`: The anti-flicker mode (Off, 50Hz, or 60Hz)
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_anti_flicker_mode(
        &self,
        mode: crate::command::exposure::AntiFlickerMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> ExposureControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::exposure::Exposure,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::ExposureCommand { mode };
        self.execute(cmd)
    }

    fn exposure_auto(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Auto)
    }

    fn exposure_manual(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Manual)
    }

    fn exposure_shutter_priority(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Shutter)
    }

    fn exposure_iris_priority(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Iris)
    }

    fn exposure_bright_mode(&self) -> M::Fut<'_, Result<(), Error>> {
        self.set_exposure_mode(crate::command::exposure::ExposureMode::Bright)
    }

    fn set_iris(&self, level: crate::types::IrisLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Iris::SetAperture(level);
        self.execute(cmd)
    }

    fn reset_iris(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Iris::Reset;
        self.execute(cmd)
    }

    fn increase_iris(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Iris::Up;
        self.execute(cmd)
    }

    fn decrease_iris(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Iris::Down;
        self.execute(cmd)
    }

    fn set_brightness(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Brightness::SetLevel(level);
        self.execute(cmd)
    }

    fn reset_brightness(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Brightness::Reset;
        self.execute(cmd)
    }

    fn increase_brightness(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Brightness::Up;
        self.execute(cmd)
    }

    fn decrease_brightness(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Brightness::Down;
        self.execute(cmd)
    }

    fn set_backlight(&self, enabled: bool) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::BacklightCommand::new(enabled);
        self.execute(cmd)
    }

    fn set_gain(&self, gain: crate::types::GainLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::gain::Gain::SetValue(gain);
        self.execute(cmd)
    }

    fn reset_gain(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::gain::Gain::Reset;
        self.execute(cmd)
    }

    fn increase_gain(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::gain::Gain::Up;
        self.execute(cmd)
    }

    fn decrease_gain(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::gain::Gain::Down;
        self.execute(cmd)
    }

    fn set_gain_limit(&self, limit: crate::types::GainLimit) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::gain::GainLimitCommand { limit };
        self.execute(cmd)
    }

    fn set_dynamic_range(
        &self,
        level: crate::types::DynamicRangeLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::DynamicRange { level };
        self.execute(cmd)
    }

    fn set_color_temperature(
        &self,
        temp: crate::types::ColorTemp,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::color::ColorTemperature::SetTemperature(temp);
        self.execute(cmd)
    }

    fn set_shutter_speed(
        &self,
        speed: crate::types::ShutterSpeed,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Shutter::SetSpeed(speed);
        self.execute(cmd)
    }

    fn reset_shutter_speed(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Shutter::Reset;
        self.execute(cmd)
    }

    fn increase_shutter_speed(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Shutter::Up;
        self.execute(cmd)
    }

    fn decrease_shutter_speed(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Shutter::Down;
        self.execute(cmd)
    }

    fn enable_spotlight(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::SpotlightOn::new();
        self.execute_updating_cache(cmd, |cache| cache.set_spotlight(true))
    }

    fn disable_spotlight(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::SpotlightOff::new();
        self.execute_updating_cache(cmd, |cache| cache.set_spotlight(false))
    }

    fn enable_auto_slow_shutter(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::AutoSlowShutterOn::new();
        self.execute_updating_cache(cmd, |cache| cache.set_auto_slow_shutter(true))
    }

    fn disable_auto_slow_shutter(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::AutoSlowShutterOff::new();
        self.execute_updating_cache(cmd, |cache| cache.set_auto_slow_shutter(false))
    }

    fn set_brightness_direct(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Brightness::Direct(level);
        self.execute(cmd)
    }

    fn set_anti_flicker_mode(
        &self,
        mode: crate::command::exposure::AntiFlickerMode,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::AntiFlickerCommand::new(mode);
        self.execute(cmd)
    }
}

/// Exposure compensation operations for cameras.
///
/// This trait provides exposure compensation control to fine-tune exposure levels
/// when the camera's automatic exposure doesn't produce the desired results.
/// Exposure compensation allows you to make the image brighter or darker
/// in incremental steps while maintaining the selected exposure mode.
///
/// # Examples
///
/// ```ignore
/// camera.enable_exposure_compensation()?;
/// camera.set_exposure_compensation_level(2)?;  // Make image brighter
/// camera.set_exposure_compensation_level(-1)?;  // Make image darker
/// camera.disable_exposure_compensation()?;
/// ```
pub trait ExposureCompensationControl {
    /// The mode type for this camera.
    type Mode: Mode;

    /// Enable exposure compensation.
    ///
    /// Enables exposure compensation which allows fine-tuning of exposure
    /// while maintaining the current exposure mode.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn enable_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable exposure compensation.
    ///
    /// Disables exposure compensation and returns to normal exposure behavior.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset exposure compensation to default.
    ///
    /// Resets exposure compensation to zero (no compensation).
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn reset_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase exposure compensation by one step.
    ///
    /// Makes the image brighter by one compensation step.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn increase_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease exposure compensation by one step.
    ///
    /// Makes the image darker by one compensation step.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn decrease_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set exposure compensation level (-7 to +7).
    ///
    /// Sets the exposure compensation to a specific level.
    /// Positive values make the image brighter, negative values make it darker.
    ///
    /// # Parameters
    /// - `level`: Compensation level from -7 (much darker) to +7 (much brighter)
    ///
    /// # Errors
    /// Returns an error if the level is out of range or if the command fails.
    fn set_exposure_compensation_level(
        &self,
        level: i8,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for ExposureCompensationControl
impl<M, P, Tr, Exec> ExposureCompensationControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasExposureCompensation,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_exposure_compensation(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::ExposureCompensation::On;
        self.execute(cmd)
    }

    fn disable_exposure_compensation(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::ExposureCompensation::Off;
        self.execute(cmd)
    }

    fn reset_exposure_compensation(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::ExposureCompensation::Reset;
        self.execute(cmd)
    }

    fn increase_exposure_compensation(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::ExposureCompensation::Up;
        self.execute(cmd)
    }

    fn decrease_exposure_compensation(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::ExposureCompensation::Down;
        self.execute(cmd)
    }

    fn set_exposure_compensation_level(&self, level: i8) -> M::Fut<'_, Result<(), Error>> {
        match crate::types::ExposureCompensationLevel::try_from(level) {
            Ok(comp_level) => {
                let cmd = crate::command::exposure::ExposureCompensation::SetLevel(comp_level);
                self.execute(cmd)
            }
            Err(e) => self.error(e),
        }
    }
}
