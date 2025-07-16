//! Async API wrapper for asynchronous camera control.
//!
//! This module provides an async interface to the camera functionality,
//! exposing only asynchronous methods.
//!
//! # Example
//!
//! ```ignore
//! use grafton_visca::r#async::{Camera, PowerOps, ZoomOps};
//! use grafton_visca::transport::tokio::Tcp;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let transport = Tcp::connect("192.168.1.100:52381").await?;
//!     let camera = grafton_visca::Camera::new(transport).r#async();
//!
//!     // Use async API
//!     camera.power_on().await?;
//!     camera.zoom_in().await?;
//!     Ok(())
//! }
//! ```

/// A newtype wrapper around the root Camera that exposes only async methods.
#[derive(Debug)]
pub struct Camera(pub(super) crate::Camera);

impl Camera {
    /// Create a new async camera wrapper.
    pub fn new(inner: crate::Camera) -> Self {
        Self(inner)
    }

    /// Create a new async camera with a custom spawner.
    ///
    /// This is a convenience method that creates a camera with a custom spawner
    /// and wraps it in the async interface. Use this when integrating with
    /// async runtimes other than Tokio.
    ///
    /// This method is only available when the `async` feature is enabled but
    /// `tokio` is not, as Tokio users can rely on the automatic runtime detection.
    ///
    /// # Example with async-std
    /// ```no_run
    /// # use grafton_visca::r#async::Camera;
    /// # use grafton_visca::executor::{Spawner, SpawnableFuture};
    /// # use grafton_visca::CameraModel;
    /// # #[cfg(all(feature = "async", not(feature = "tokio")))]
    /// # #[async_std::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #[derive(Clone)]
    /// struct AsyncStdSpawner;
    ///
    /// impl Spawner for AsyncStdSpawner {
    ///     fn spawn(&self, task: SpawnableFuture) {
    ///         async_std::task::spawn(task);
    ///     }
    /// }
    ///
    /// # let transport = todo!();
    /// let spawner = AsyncStdSpawner;
    /// let camera = Camera::with_spawner(CameraModel::PTZOpticsG2, transport, spawner);
    ///
    /// // Ready to control camera with async-std
    /// camera.pan_tilt_home().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(all(feature = "async", not(feature = "tokio")))]
    pub fn with_spawner<T, S>(profile: crate::CameraModel, transport: T, spawner: S) -> Self
    where
        T: crate::transport::core::Transport + Send + Sync + 'static,
        S: crate::executor::Spawner,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let inner = crate::Camera::with_profile_and_spawner(profile, transport, spawner);
        Self(inner)
    }
}

/// Prelude for async camera operations.
///
/// Import this to get all async trait operations:
/// ```ignore
/// use grafton_visca::r#async::prelude::*;
/// ```
pub mod prelude {
    pub use super::{
        ColorOps, ExposureOps, FocusOps, ImageProcessingOps, InquiryOps, NDFilterOps,
        PanTiltInquiryOps, PanTiltOps, PowerOps, PresetsOps, SystemOps, TallyOps, 
        WhiteBalanceOps, ZoomOps,
    };
}

// Re-export async traits with unsuffixed names
pub use crate::camera::methods::{
    ColorOps, ExposureOps, FocusOps, ImageProcessingOps, InquiryOps, NDFilterOps,
    PanTiltInquiryOps, PanTiltOps, PowerOps, PresetsOps, SystemOps, TallyOps, WhiteBalanceOps,
    ZoomOps,
};

// Implement all async traits for the wrapper type
impl ZoomOps for Camera {
    async fn zoom_stop(&self) -> crate::Result<()> {
        self.0.zoom_stop().await
    }

    async fn zoom_in(&self) -> crate::Result<()> {
        self.0.zoom_in().await
    }

    async fn zoom_out(&self) -> crate::Result<()> {
        self.0.zoom_out().await
    }

    async fn zoom_absolute(&self, position: crate::units::Normalized) -> crate::Result<()> {
        self.0.zoom_absolute(position).await
    }
}

impl ColorOps for Camera {
    async fn one_push_trigger(&self) -> crate::Result<()> {
        self.0.one_push_trigger().await
    }

    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> crate::Result<()> {
        ColorOps::set_color_temperature(&self.0, temp).await
    }

    async fn color_temperature(&self, temp: Option<crate::types::ColorTemp>) -> crate::Result<()> {
        self.0.color_temperature(temp).await
    }

    async fn set_red_gain(&self, gain: crate::types::RedChannel) -> crate::Result<()> {
        self.0.set_red_gain(gain).await
    }

    async fn red_gain(&self, command: crate::command::RedGain) -> crate::Result<()> {
        self.0.red_gain(command).await
    }

    async fn set_blue_gain(&self, gain: crate::types::BlueChannel) -> crate::Result<()> {
        self.0.set_blue_gain(gain).await
    }

    async fn blue_gain(&self, command: crate::command::BlueGain) -> crate::Result<()> {
        self.0.blue_gain(command).await
    }

    async fn set_red_tuning(&self, tuning: crate::types::RedTuning) -> crate::Result<()> {
        self.0.set_red_tuning(tuning).await
    }

    async fn set_blue_tuning(&self, tuning: crate::types::BlueTuning) -> crate::Result<()> {
        self.0.set_blue_tuning(tuning).await
    }
}

impl ExposureOps for Camera {
    async fn exposure_auto(&self) -> crate::Result<()> {
        self.0.exposure_auto().await
    }

    async fn exposure_manual(&self) -> crate::Result<()> {
        self.0.exposure_manual().await
    }

    async fn set_iris(&self, level: crate::types::IrisLevel) -> crate::Result<()> {
        self.0.set_iris(level).await
    }

    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> crate::Result<()> {
        self.0.set_brightness(level).await
    }

    async fn set_backlight(&self, enabled: bool) -> crate::Result<()> {
        self.0.set_backlight(enabled).await
    }

    async fn set_gain(&self, gain: crate::types::GainLevel) -> crate::Result<()> {
        self.0.set_gain(gain).await
    }

    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> crate::Result<()> {
        self.0.set_gain_limit(limit).await
    }

    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> crate::Result<()> {
        self.0.set_dynamic_range(level).await
    }

    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> crate::Result<()> {
        ExposureOps::set_color_temperature(&self.0, temp).await
    }
}

impl FocusOps for Camera {
    async fn focus_auto(&self) -> crate::Result<()> {
        self.0.focus_auto().await
    }

    async fn focus_manual(&self) -> crate::Result<()> {
        self.0.focus_manual().await
    }

    async fn focus_near(&self, speed: crate::types::SpeedLevel) -> crate::Result<()> {
        self.0.focus_near(speed).await
    }

    async fn focus_far(&self, speed: crate::types::SpeedLevel) -> crate::Result<()> {
        self.0.focus_far(speed).await
    }

    async fn focus_stop(&self) -> crate::Result<()> {
        self.0.focus_stop().await
    }

    async fn focus_one_push(&self) -> crate::Result<()> {
        self.0.focus_one_push().await
    }

    async fn set_focus(&self, position: crate::types::FocusPosition) -> crate::Result<()> {
        self.0.set_focus(position).await
    }
}

impl ImageProcessingOps for Camera {
    async fn enable_flip(&self) -> crate::Result<()> {
        self.0.enable_flip().await
    }

    async fn set_contrast(&self, level: crate::types::ContrastLevel) -> crate::Result<()> {
        self.0.set_contrast(level).await
    }

    async fn set_sharpness(&self, level: crate::types::SharpnessLevel) -> crate::Result<()> {
        self.0.set_sharpness(level).await
    }

    async fn set_saturation(&self, level: crate::types::SaturationLevel) -> crate::Result<()> {
        self.0.set_saturation(level).await
    }

    async fn set_hue(&self, level: crate::types::HueLevel) -> crate::Result<()> {
        self.0.set_hue(level).await
    }

    async fn set_noise_reduction_2d(
        &self,
        level: crate::types::NoiseReduction2DLevel,
    ) -> crate::Result<()> {
        self.0.set_noise_reduction_2d(level).await
    }

    async fn set_noise_reduction_3d(
        &self,
        level: crate::types::NoiseReduction3DLevel,
    ) -> crate::Result<()> {
        self.0.set_noise_reduction_3d(level).await
    }

    async fn set_image_flip(&self, mode: crate::command::ImageFlipMode) -> crate::Result<()> {
        self.0.set_image_flip(mode).await
    }

    async fn set_luminance(&self, level: crate::types::LuminanceLevel) -> crate::Result<()> {
        self.0.set_luminance(level).await
    }
}

impl InquiryOps for Camera {
    async fn get_power_state(&self) -> crate::Result<bool> {
        self.0.get_power_state().await
    }

    async fn get_zoom_position(&self) -> crate::Result<u16> {
        self.0.get_zoom_position().await
    }

    async fn get_focus_position(&self) -> crate::Result<u16> {
        self.0.get_focus_position().await
    }

    async fn get_focus_near_limit(&self) -> crate::Result<u16> {
        self.0.get_focus_near_limit().await
    }

    async fn get_focus_zone(&self) -> crate::Result<crate::command::FocusZone> {
        self.0.get_focus_zone().await
    }

    async fn get_auto_focus_sensitivity(
        &self,
    ) -> crate::Result<crate::command::AutoFocusSensitivity> {
        self.0.get_auto_focus_sensitivity().await
    }

    async fn get_exposure_mode(&self) -> crate::Result<crate::command::ExposureMode> {
        self.0.get_exposure_mode().await
    }

    async fn get_exposure_compensation(&self) -> crate::Result<i8> {
        self.0.get_exposure_compensation().await
    }

    async fn get_exposure_compensation_enabled(&self) -> crate::Result<bool> {
        self.0.get_exposure_compensation_enabled().await
    }

    async fn get_iris(&self) -> crate::Result<u8> {
        self.0.get_iris().await
    }

    async fn get_shutter(&self) -> crate::Result<u16> {
        self.0.get_shutter().await
    }

    async fn get_gain(&self) -> crate::Result<u8> {
        self.0.get_gain().await
    }

    async fn get_gain_limit(&self) -> crate::Result<u8> {
        self.0.get_gain_limit().await
    }

    async fn get_white_balance_mode(&self) -> crate::Result<crate::command::WhiteBalanceMode> {
        self.0.get_white_balance_mode().await
    }

    async fn get_red_gain(&self) -> crate::Result<u8> {
        self.0.get_red_gain().await
    }

    async fn get_blue_gain(&self) -> crate::Result<u8> {
        self.0.get_blue_gain().await
    }

    async fn get_red_tuning(&self) -> crate::Result<u8> {
        self.0.get_red_tuning().await
    }

    async fn get_blue_tuning(&self) -> crate::Result<u8> {
        self.0.get_blue_tuning().await
    }

    async fn get_color_temperature(&self) -> crate::Result<u16> {
        self.0.get_color_temperature().await
    }

    async fn get_anti_flicker(&self) -> crate::Result<crate::command::AntiFlickerMode> {
        self.0.get_anti_flicker().await
    }

    async fn get_gamma(&self) -> crate::Result<u8> {
        self.0.get_gamma().await
    }

    async fn get_contrast(&self) -> crate::Result<u8> {
        self.0.get_contrast().await
    }

    async fn get_brightness(&self) -> crate::Result<u8> {
        self.0.get_brightness().await
    }

    async fn get_sharpness(&self) -> crate::Result<u8> {
        self.0.get_sharpness().await
    }

    async fn get_sharpness_mode(&self) -> crate::Result<crate::command::SharpnessMode> {
        self.0.get_sharpness_mode().await
    }

    async fn get_saturation(&self) -> crate::Result<u8> {
        self.0.get_saturation().await
    }

    async fn get_hue(&self) -> crate::Result<u8> {
        self.0.get_hue().await
    }

    async fn get_noise_reduction_2d(&self) -> crate::Result<u8> {
        self.0.get_noise_reduction_2d().await
    }

    async fn get_noise_reduction_3d(&self) -> crate::Result<u8> {
        self.0.get_noise_reduction_3d().await
    }

    async fn get_black_white(&self) -> crate::Result<bool> {
        self.0.get_black_white().await
    }
}

impl NDFilterOps for Camera {
    async fn set_nd_filter(&self, level: u8) -> crate::Result<()> {
        self.0.set_nd_filter(level).await
    }

    async fn get_nd_filter(&self) -> crate::Result<u8> {
        self.0.get_nd_filter().await
    }
}

impl PanTiltOps for Camera {
    async fn pan_tilt_stop(&self) -> crate::Result<()> {
        self.0.pan_tilt_stop().await
    }

    async fn pan_tilt_home(&self) -> crate::Result<()> {
        self.0.pan_tilt_home().await
    }

    async fn pan_tilt_absolute(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> crate::Result<()> {
        self.0.pan_tilt_absolute(pan, tilt, speed).await
    }

    async fn pan_tilt_relative(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> crate::Result<()> {
        self.0.pan_tilt_relative(pan, tilt, speed).await
    }

    async fn pan_tilt_move(
        &self,
        direction: crate::command::PanTiltDirection,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> crate::Result<()> {
        self.0.pan_tilt_move(direction, pan_speed, tilt_speed).await
    }

    async fn pan_tilt_reset(&self) -> crate::Result<()> {
        self.0.pan_tilt_reset().await
    }
}

impl PanTiltInquiryOps for Camera {
    async fn get_pan_tilt_position(&self) -> crate::Result<(i16, i16)> {
        self.0.get_pan_tilt_position().await
    }

    async fn get_pan_tilt_degrees(
        &self,
    ) -> crate::Result<(crate::units::Degrees, crate::units::Degrees)> {
        self.0.get_pan_tilt_degrees().await
    }
}

impl PowerOps for Camera {
    async fn power_on(&self) -> crate::Result<()> {
        self.0.power_on().await
    }

    async fn power_off(&self) -> crate::Result<()> {
        self.0.power_off().await
    }
}

impl PresetsOps for Camera {
    async fn preset_recall(&self, preset: crate::command::PresetNumber) -> crate::Result<()> {
        self.0.preset_recall(preset).await
    }

    async fn preset_set(&self, preset: crate::command::PresetNumber) -> crate::Result<()> {
        self.0.preset_set(preset).await
    }
}

impl SystemOps for Camera {
    async fn trigger_address_assignment(&self) -> crate::Result<()> {
        self.0.trigger_address_assignment().await
    }

    async fn interface_clear(&self) -> crate::Result<()> {
        self.0.interface_clear().await
    }

    async fn cancel_command(&self, socket: crate::command::Socket) -> crate::Result<()> {
        self.0.cancel_command(socket).await
    }
}

impl TallyOps for Camera {
    async fn tally_red_on(&self) -> crate::Result<()> {
        self.0.tally_red_on().await
    }

    async fn tally_red_off(&self) -> crate::Result<()> {
        self.0.tally_red_off().await
    }

    async fn tally_bright_lo(&self) -> crate::Result<()> {
        self.0.tally_bright_lo().await
    }

    async fn tally_bright_hi(&self) -> crate::Result<()> {
        self.0.tally_bright_hi().await
    }

    async fn tally_green_on(&self) -> crate::Result<()> {
        self.0.tally_green_on().await
    }

    async fn tally_green_off(&self) -> crate::Result<()> {
        self.0.tally_green_off().await
    }

    async fn tally_flash(&self) -> crate::Result<()> {
        self.0.tally_flash().await
    }

    async fn tally_on(&self) -> crate::Result<()> {
        self.0.tally_on().await
    }

    async fn tally_off(&self) -> crate::Result<()> {
        self.0.tally_off().await
    }

    async fn get_tally_status(&self) -> crate::Result<bool> {
        self.0.get_tally_status().await
    }
}

impl WhiteBalanceOps for Camera {
    async fn white_balance_auto(&self) -> crate::Result<()> {
        self.0.white_balance_auto().await
    }
}
