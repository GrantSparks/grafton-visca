//! exposure control implementation using Mode trait.

use crate::{camera::ViscaClient, mode::Mode, Error};

/// exposure operations for cameras.
///
/// This trait provides exposure control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait ExposureControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set exposure mode to any supported mode.
    fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set auto exposure mode.
    fn exposure_auto(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set manual exposure mode.
    fn exposure_manual(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set shutter priority exposure mode.
    fn exposure_shutter_priority(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set iris priority exposure mode.
    fn exposure_iris_priority(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set brightness priority exposure mode.
    fn exposure_bright_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set iris level.
    fn set_iris(
        &self,
        level: crate::types::IrisLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset iris to default.
    fn reset_iris(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase iris (open aperture).
    fn increase_iris(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease iris (close aperture).
    fn decrease_iris(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set brightness level.
    fn set_brightness(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset brightness to default.
    fn reset_brightness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase brightness.
    fn increase_brightness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease brightness.
    fn decrease_brightness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set backlight compensation.
    fn set_backlight(&self, enabled: bool) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set gain value.
    fn set_gain(
        &self,
        gain: crate::types::GainLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset gain to default.
    fn reset_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase gain by one step.
    fn increase_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease gain by one step.
    fn decrease_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set gain limit.
    fn set_gain_limit(
        &self,
        limit: crate::types::GainLimit,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set dynamic range level.
    fn set_dynamic_range(
        &self,
        level: crate::types::DynamicRangeLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set color temperature.
    fn set_color_temperature(
        &self,
        temp: crate::types::ColorTemp,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set shutter speed.
    fn set_shutter_speed(
        &self,
        speed: crate::types::ShutterSpeed,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset shutter speed to default.
    fn reset_shutter_speed(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase shutter speed (faster).
    fn increase_shutter_speed(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease shutter speed (slower).
    fn decrease_shutter_speed(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable spotlight mode (Sony models).
    fn enable_spotlight(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable spotlight mode (Sony models).
    fn disable_spotlight(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable auto slow shutter mode.
    fn enable_auto_slow_shutter(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable auto slow shutter mode.
    fn disable_auto_slow_shutter(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set brightness using direct mode.
    fn set_brightness_direct(
        &self,
        level: crate::types::BrightnessLevel,
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

    /// Set the color temperature.
    ///
    /// **Note:** This operation is currently not supported and will always return
    /// `Error::NotSupported`. Setting color temperature requires sending multiple
    /// commands sequentially, which is not yet implemented.
    fn set_color_temperature(
        &self,
        _temp: crate::types::ColorTemp,
    ) -> M::Fut<'_, Result<(), Error>> {
        // TODO: This requires sending two commands sequentially
        // For now, return unsupported
        self.error(Error::NotSupported)
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
        self.execute(cmd)
    }

    fn disable_spotlight(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::SpotlightOff::new();
        self.execute(cmd)
    }

    fn enable_auto_slow_shutter(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::AutoSlowShutterOn::new();
        self.execute(cmd)
    }

    fn disable_auto_slow_shutter(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::AutoSlowShutterOff::new();
        self.execute(cmd)
    }

    fn set_brightness_direct(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::exposure::Brightness::Direct(level);
        self.execute(cmd)
    }
}

/// Exposure compensation operations for cameras.
pub trait ExposureCompensationControl {
    /// The mode type for this camera.
    type Mode: Mode;

    /// Enable exposure compensation.
    fn enable_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable exposure compensation.
    fn disable_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset exposure compensation to default.
    fn reset_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase exposure compensation by one step.
    fn increase_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease exposure compensation by one step.
    fn decrease_exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set exposure compensation level (-7 to +7).
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
