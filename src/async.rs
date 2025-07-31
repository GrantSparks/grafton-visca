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
//!     let camera = grafton_visca::r#async::Camera::new(grafton_visca::Camera::new(transport));
//!
//!     // Use async API
//!     camera.power_on().await?;
//!     camera.zoom_in().await?;
//!     Ok(())
//! }
//! ```

// Import only the traits we're implementing delegates for
// These are needed for the delegation implementations
#[cfg(feature = "async")]
use crate::camera::methods::{ColorOps as CameraColorOps, ExposureOps as CameraExposureOps};

/// A newtype wrapper around the root Camera that exposes only async methods.
#[derive(Debug)]
pub struct Camera<P, T>(pub(super) crate::Camera<P, T>)
where
    P: crate::capabilities::Profile,
    T: crate::transport::UnifiedTransport;

impl<P, T> Camera<P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::UnifiedTransport,
{
    /// Create a new async camera wrapper.
    #[must_use]
    pub fn new(inner: crate::Camera<P, T>) -> Self {
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
    #[cfg(feature = "async")]
    pub use crate::camera::methods::{
        ColorOps, ExposureCompensationOps, ExposureOps, FocusOps, ImageProcessingOps, InquiryOps,
        MenuControlOps, MotionSyncControl, NDFilterOps, PanTiltInquiryOps, PanTiltOps, PowerOps,
        PresetsOps, StreamingOps, SystemOps, TallyOps, VariableSpeedOps, WhiteBalanceOps, ZoomOps,
    };
}

// Re-export async traits with unsuffixed names
#[cfg(feature = "async")]
pub use crate::camera::methods::{
    ColorOps, ExposureCompensationOps, ExposureOps, FocusOps, ImageProcessingOps, InquiryOps,
    MenuControlOps, MotionSyncControl, NDFilterOps, PanTiltInquiryOps, PanTiltOps, PowerOps,
    PresetsOps, StreamingOps, SystemOps, TallyOps, VariableSpeedOps, WhiteBalanceOps, ZoomOps,
};

// Implement all async traits for the wrapper type
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> ZoomOps
    for Camera<P, T>
{
    async fn zoom_stop(&self) -> crate::Result<()> {
        self.0.zoom_stop().await
    }

    async fn zoom_in(&self) -> crate::Result<()> {
        self.0.zoom_in().await
    }

    async fn zoom_out(&self) -> crate::Result<()> {
        self.0.zoom_out().await
    }

    async fn zoom_in_standard(&self) -> crate::Result<()> {
        self.0.zoom_in_standard().await
    }

    async fn zoom_out_standard(&self) -> crate::Result<()> {
        self.0.zoom_out_standard().await
    }

    async fn zoom_absolute(&self, position: crate::units::Normalized) -> crate::Result<()> {
        self.0.zoom_absolute(position).await
    }

    async fn enable_digital_zoom(&self) -> crate::Result<()> {
        self.0.enable_digital_zoom().await
    }

    async fn disable_digital_zoom(&self) -> crate::Result<()> {
        self.0.disable_digital_zoom().await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> ColorOps
    for Camera<P, T>
{
    async fn one_push_trigger(&self) -> crate::Result<()> {
        self.0.one_push_trigger().await
    }

    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> crate::Result<()> {
        CameraColorOps::set_color_temperature(&self.0, temp).await
    }

    async fn reset_color_temperature(&self) -> crate::Result<()> {
        self.0.reset_color_temperature().await
    }

    async fn increase_color_temperature(&self) -> crate::Result<()> {
        self.0.increase_color_temperature().await
    }

    async fn decrease_color_temperature(&self) -> crate::Result<()> {
        self.0.decrease_color_temperature().await
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

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> ExposureOps
    for Camera<P, T>
{
    async fn set_exposure_mode(
        &self,
        mode: crate::command::exposure::ExposureMode,
    ) -> crate::Result<()> {
        self.0.set_exposure_mode(mode).await
    }

    async fn exposure_auto(&self) -> crate::Result<()> {
        self.0.exposure_auto().await
    }

    async fn exposure_manual(&self) -> crate::Result<()> {
        self.0.exposure_manual().await
    }

    async fn exposure_shutter_priority(&self) -> crate::Result<()> {
        self.0.exposure_shutter_priority().await
    }

    async fn exposure_iris_priority(&self) -> crate::Result<()> {
        self.0.exposure_iris_priority().await
    }

    async fn exposure_bright_mode(&self) -> crate::Result<()> {
        self.0.exposure_bright_mode().await
    }

    async fn set_iris(&self, level: crate::types::IrisLevel) -> crate::Result<()> {
        self.0.set_iris(level).await
    }

    async fn reset_iris(&self) -> crate::Result<()> {
        self.0.reset_iris().await
    }

    async fn increase_iris(&self) -> crate::Result<()> {
        self.0.increase_iris().await
    }

    async fn decrease_iris(&self) -> crate::Result<()> {
        self.0.decrease_iris().await
    }

    async fn set_brightness(&self, level: crate::types::BrightnessLevel) -> crate::Result<()> {
        self.0.set_brightness(level).await
    }

    async fn reset_brightness(&self) -> crate::Result<()> {
        self.0.reset_brightness().await
    }

    async fn increase_brightness(&self) -> crate::Result<()> {
        self.0.increase_brightness().await
    }

    async fn decrease_brightness(&self) -> crate::Result<()> {
        self.0.decrease_brightness().await
    }

    async fn set_backlight(&self, enabled: bool) -> crate::Result<()> {
        self.0.set_backlight(enabled).await
    }

    async fn set_gain(&self, gain: crate::types::GainLevel) -> crate::Result<()> {
        self.0.set_gain(gain).await
    }

    async fn reset_gain(&self) -> crate::Result<()> {
        self.0.reset_gain().await
    }

    async fn increase_gain(&self) -> crate::Result<()> {
        self.0.increase_gain().await
    }

    async fn decrease_gain(&self) -> crate::Result<()> {
        self.0.decrease_gain().await
    }

    async fn set_gain_limit(&self, limit: crate::types::GainLimit) -> crate::Result<()> {
        self.0.set_gain_limit(limit).await
    }

    async fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> crate::Result<()> {
        self.0.set_dynamic_range(level).await
    }

    async fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> crate::Result<()> {
        CameraExposureOps::set_color_temperature(&self.0, temp).await
    }

    async fn set_shutter_speed(&self, speed: crate::types::ShutterSpeed) -> crate::Result<()> {
        self.0.set_shutter_speed(speed).await
    }

    async fn reset_shutter_speed(&self) -> crate::Result<()> {
        self.0.reset_shutter_speed().await
    }

    async fn increase_shutter_speed(&self) -> crate::Result<()> {
        self.0.increase_shutter_speed().await
    }

    async fn decrease_shutter_speed(&self) -> crate::Result<()> {
        self.0.decrease_shutter_speed().await
    }

    async fn enable_spotlight(&self) -> crate::Result<()> {
        self.0.enable_spotlight().await
    }

    async fn disable_spotlight(&self) -> crate::Result<()> {
        self.0.disable_spotlight().await
    }

    async fn enable_auto_slow_shutter(&self) -> crate::Result<()> {
        self.0.enable_auto_slow_shutter().await
    }

    async fn disable_auto_slow_shutter(&self) -> crate::Result<()> {
        self.0.disable_auto_slow_shutter().await
    }

    async fn set_brightness_direct(
        &self,
        level: crate::types::BrightnessLevel,
    ) -> crate::Result<()> {
        self.0.set_brightness_direct(level).await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> FocusOps
    for Camera<P, T>
{
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

    async fn focus_infinity(&self) -> crate::Result<()> {
        self.0.focus_infinity().await
    }

    async fn enable_focus_lock(&self) -> crate::Result<()> {
        self.0.enable_focus_lock().await
    }

    async fn disable_focus_lock(&self) -> crate::Result<()> {
        self.0.disable_focus_lock().await
    }

    async fn push_af_press(&self) -> crate::Result<()> {
        self.0.push_af_press().await
    }

    async fn push_af_release(&self) -> crate::Result<()> {
        self.0.push_af_release().await
    }

    async fn set_focus_zone(&self, zone: crate::command::focus::FocusZone) -> crate::Result<()> {
        self.0.set_focus_zone(zone).await
    }

    async fn set_auto_focus_sensitivity(
        &self,
        sensitivity: crate::command::focus::AutoFocusSensitivity,
    ) -> crate::Result<()> {
        self.0.set_auto_focus_sensitivity(sensitivity).await
    }

    async fn set_focus_near_limit(
        &self,
        position: crate::types::FocusPosition,
    ) -> crate::Result<()> {
        self.0.set_focus_near_limit(position).await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> ImageProcessingOps
    for Camera<P, T>
{
    async fn enable_flip(&self) -> crate::Result<()> {
        self.0.enable_flip().await
    }

    async fn disable_flip(&self) -> crate::Result<()> {
        self.0.disable_flip().await
    }

    async fn enable_horizontal_flip(&self) -> crate::Result<()> {
        self.0.enable_horizontal_flip().await
    }

    async fn disable_horizontal_flip(&self) -> crate::Result<()> {
        self.0.disable_horizontal_flip().await
    }

    async fn set_contrast(&self, level: crate::types::ContrastLevel) -> crate::Result<()> {
        self.0.set_contrast(level).await
    }

    async fn set_sharpness(&self, level: crate::types::SharpnessLevel) -> crate::Result<()> {
        self.0.set_sharpness(level).await
    }

    async fn set_sharpness_mode(&self, mode: crate::command::SharpnessMode) -> crate::Result<()> {
        self.0.set_sharpness_mode(mode).await
    }

    async fn reset_sharpness(&self) -> crate::Result<()> {
        self.0.reset_sharpness().await
    }

    async fn increase_sharpness(&self) -> crate::Result<()> {
        self.0.increase_sharpness().await
    }

    async fn decrease_sharpness(&self) -> crate::Result<()> {
        self.0.decrease_sharpness().await
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

    async fn disable_noise_reduction_2d(&self) -> crate::Result<()> {
        self.0.disable_noise_reduction_2d().await
    }

    async fn set_noise_reduction_3d(
        &self,
        level: crate::types::NoiseReduction3DLevel,
    ) -> crate::Result<()> {
        self.0.set_noise_reduction_3d(level).await
    }

    async fn disable_noise_reduction_3d(&self) -> crate::Result<()> {
        self.0.disable_noise_reduction_3d().await
    }

    async fn set_image_flip(&self, mode: crate::command::ImageFlipMode) -> crate::Result<()> {
        self.0.set_image_flip(mode).await
    }

    async fn set_luminance(&self, level: crate::types::LuminanceLevel) -> crate::Result<()> {
        self.0.set_luminance(level).await
    }

    async fn enable_freeze(&self) -> crate::Result<()> {
        self.0.enable_freeze().await
    }

    async fn disable_freeze(&self) -> crate::Result<()> {
        self.0.disable_freeze().await
    }

    async fn enable_black_white(&self) -> crate::Result<()> {
        self.0.enable_black_white().await
    }

    async fn disable_black_white(&self) -> crate::Result<()> {
        self.0.disable_black_white().await
    }

    async fn set_picture_effect(&self, mode: crate::PictureEffectMode) -> crate::Result<()> {
        self.0.set_picture_effect(mode).await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> InquiryOps
    for Camera<P, T>
{
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

    async fn get_resolution(&self) -> crate::Result<crate::command::resolution::ResolutionMode> {
        self.0.get_resolution().await
    }

    async fn get_picture_effect(
        &self,
    ) -> crate::Result<crate::command::resolution::PictureEffectMode> {
        self.0.get_picture_effect().await
    }

    async fn get_nd_filter_position(
        &self,
    ) -> crate::Result<crate::command::resolution::NDFilterPosition> {
        self.0.get_nd_filter_position().await
    }

    async fn get_version(&self) -> crate::Result<crate::command::Version> {
        self.0.get_version().await
    }

    async fn get_luminance(&self) -> crate::Result<u8> {
        self.0.get_luminance().await
    }

    async fn get_backlight_enabled(&self) -> crate::Result<bool> {
        self.0.get_backlight_enabled().await
    }

    async fn get_image_flip(&self) -> crate::Result<crate::command::ImageFlipStatus> {
        self.0.get_image_flip().await
    }

    async fn get_dynamic_range(&self) -> crate::Result<u8> {
        self.0.get_dynamic_range().await
    }

    async fn get_focus_mode(&self) -> crate::Result<crate::command::FocusMode> {
        self.0.get_focus_mode().await
    }

    async fn get_menu_status(&self) -> crate::Result<bool> {
        self.0.get_menu_status().await
    }

    async fn get_auto_focus_enabled(&self) -> crate::Result<bool> {
        self.0.get_auto_focus_enabled().await
    }

    async fn get_tally_light_status(&self) -> crate::Result<crate::command::TallyStatus> {
        self.0.get_tally_light_status().await
    }

    async fn get_night_day_mode(&self) -> crate::Result<crate::command::NightDayMode> {
        self.0.get_night_day_mode().await
    }

    async fn get_flip_mode(&self) -> crate::Result<crate::command::FlipMode> {
        self.0.get_flip_mode().await
    }

    async fn get_standby_enabled(&self) -> crate::Result<bool> {
        self.0.get_standby_enabled().await
    }

    async fn get_focus_range(&self) -> crate::Result<crate::command::FocusRange> {
        self.0.get_focus_range().await
    }

    async fn get_iris_control(&self) -> crate::Result<crate::command::IrisControl> {
        self.0.get_iris_control().await
    }

    async fn get_defog_mode(&self) -> crate::Result<bool> {
        self.0.get_defog_mode().await
    }

    async fn get_defog_level(&self) -> crate::Result<u8> {
        self.0.get_defog_level().await
    }

    async fn get_digital_ptz_enabled(&self) -> crate::Result<bool> {
        self.0.get_digital_ptz_enabled().await
    }

    async fn get_auto_white_balance_sensitivity(
        &self,
    ) -> crate::Result<crate::command::AutoWhiteBalanceSensitivity> {
        self.0.get_auto_white_balance_sensitivity().await
    }

    async fn get_exposure_compensation_position(&self) -> crate::Result<u16> {
        self.0.get_exposure_compensation_position().await
    }

    async fn get_auto_trace_enabled(&self) -> crate::Result<bool> {
        self.0.get_auto_trace_enabled().await
    }

    async fn get_focus_unlock(&self) -> crate::Result<bool> {
        self.0.get_focus_unlock().await
    }

    async fn get_sharpness_position(&self) -> crate::Result<u16> {
        self.0.get_sharpness_position().await
    }

    async fn get_noise_reduction_level(&self) -> crate::Result<u8> {
        self.0.get_noise_reduction_level().await
    }

    async fn get_broadcast_domain(&self) -> crate::Result<u8> {
        self.0.get_broadcast_domain().await
    }

    async fn get_noise_reduction_mode(&self) -> crate::Result<crate::command::NrMode> {
        self.0.get_noise_reduction_mode().await
    }

    async fn get_noise_reduction_speed(&self) -> crate::Result<crate::command::NrSpeed> {
        self.0.get_noise_reduction_speed().await
    }

    async fn get_black_white_mode(&self) -> crate::Result<crate::command::BlackWhiteMode> {
        self.0.get_black_white_mode().await
    }

    async fn get_usb_audio_enabled(&self) -> crate::Result<bool> {
        self.0.get_usb_audio_enabled().await
    }

    async fn get_two_tone_mode_enabled(&self) -> crate::Result<bool> {
        self.0.get_two_tone_mode_enabled().await
    }

    async fn get_nd_filter_preset(&self) -> crate::Result<u8> {
        self.0.get_nd_filter_preset().await
    }

    async fn get_digital_mode_enabled(&self) -> crate::Result<bool> {
        self.0.get_digital_mode_enabled().await
    }

    async fn get_tally_auto_adjust_enabled(&self) -> crate::Result<bool> {
        self.0.get_tally_auto_adjust_enabled().await
    }

    async fn get_tally_green_enabled(&self) -> crate::Result<bool> {
        self.0.get_tally_green_enabled().await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> NDFilterOps
    for Camera<P, T>
{
    async fn set_nd_filter_mode(&self, mode: crate::command::NDFilterMode) -> crate::Result<()> {
        self.0.set_nd_filter_mode(mode).await
    }

    async fn set_nd_filter_value(&self, value: u16) -> crate::Result<()> {
        self.0.set_nd_filter_value(value).await
    }

    async fn set_nd_filter_stops(&self, stops: f32) -> crate::Result<()> {
        self.0.set_nd_filter_stops(stops).await
    }

    async fn step_nd_filter(&self, direction: crate::command::NDFilterStep) -> crate::Result<()> {
        self.0.step_nd_filter(direction).await
    }

    async fn set_auto_nd(&self, enabled: bool) -> crate::Result<()> {
        self.0.set_auto_nd(enabled).await
    }

    async fn get_nd_filter(&self) -> crate::Result<u8> {
        self.0.get_nd_filter().await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> MotionSyncControl
    for Camera<P, T>
{
    async fn set_motion_sync_mode(&self, mode: crate::MotionSyncMode) -> crate::Result<()> {
        self.0.set_motion_sync_mode(mode).await
    }

    async fn set_motion_sync_speed(&self, speed: u8) -> crate::Result<()> {
        self.0.set_motion_sync_speed(speed).await
    }

    async fn set_motion_sync_preset_speed(
        &self,
        speed: crate::MotionSyncSpeed,
    ) -> crate::Result<()> {
        self.0.set_motion_sync_preset_speed(speed).await
    }

    async fn get_motion_sync_mode(&self) -> crate::Result<crate::MotionSyncMode> {
        self.0.get_motion_sync_mode().await
    }

    async fn get_motion_sync_speed(&self) -> crate::Result<crate::MotionSyncSpeed> {
        self.0.get_motion_sync_speed().await
    }
}

#[async_trait::async_trait]
impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> MenuControlOps
    for Camera<P, T>
{
    async fn set_menu_display(&self, display: bool) -> crate::Result<crate::command::Response> {
        self.0.set_menu_display(display).await
    }

    async fn menu_navigate(
        &self,
        direction: crate::command::MenuDirection,
    ) -> crate::Result<crate::command::Response> {
        self.0.menu_navigate(direction).await
    }

    async fn menu_action(
        &self,
        action: crate::command::MenuAction,
    ) -> crate::Result<crate::command::Response> {
        self.0.menu_action(action).await
    }

    async fn direct_menu_control(
        &self,
        control1: u8,
        control2: u8,
    ) -> crate::Result<crate::command::Response> {
        self.0.direct_menu_control(control1, control2).await
    }

    async fn toggle_menu(&self) -> crate::Result<crate::command::Response> {
        self.0.toggle_menu().await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> PanTiltOps
    for Camera<P, T>
{
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

    async fn pan_tilt_limit_set(
        &self,
        corner: crate::command::pan_tilt::PanTiltLimitCorner,
        pan: crate::types::PanPosition,
        tilt: crate::types::TiltPosition,
    ) -> crate::Result<()> {
        self.0.pan_tilt_limit_set(corner, pan, tilt).await
    }

    async fn pan_tilt_limit_clear(
        &self,
        corner: crate::command::pan_tilt::PanTiltLimitCorner,
    ) -> crate::Result<()> {
        self.0.pan_tilt_limit_clear(corner).await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> PanTiltInquiryOps
    for Camera<P, T>
{
    async fn get_pan_tilt_position(&self) -> crate::Result<(i16, i16)> {
        self.0.get_pan_tilt_position().await
    }

    async fn get_pan_tilt_degrees(
        &self,
    ) -> crate::Result<(crate::units::Degrees, crate::units::Degrees)> {
        self.0.get_pan_tilt_degrees().await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> PowerOps
    for Camera<P, T>
{
    async fn power_on(&self) -> crate::Result<()> {
        self.0.power_on().await
    }

    async fn power_off(&self) -> crate::Result<()> {
        self.0.power_off().await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> PresetsOps
    for Camera<P, T>
{
    async fn preset_recall(&self, preset: crate::command::PresetNumber) -> crate::Result<()> {
        self.0.preset_recall(preset).await
    }

    async fn preset_set(&self, preset: crate::command::PresetNumber) -> crate::Result<()> {
        self.0.preset_set(preset).await
    }

    async fn preset_reset(&self, preset: crate::command::PresetNumber) -> crate::Result<()> {
        self.0.preset_reset(preset).await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> SystemOps
    for Camera<P, T>
{
    async fn trigger_address_assignment(&self) -> crate::Result<()> {
        self.0.trigger_address_assignment().await
    }

    async fn interface_clear(&self) -> crate::Result<()> {
        self.0.interface_clear().await
    }

    async fn cancel_command(&self, socket: crate::command::system::Socket) -> crate::Result<()> {
        self.0.cancel_command(socket).await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> TallyOps
    for Camera<P, T>
{
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

    async fn get_red_tally_status(&self) -> crate::Result<bool> {
        self.0.get_red_tally_status().await
    }

    async fn get_green_tally_status(&self) -> crate::Result<bool> {
        self.0.get_green_tally_status().await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> WhiteBalanceOps
    for Camera<P, T>
{
    async fn set_white_balance_mode(
        &self,
        mode: crate::command::white_balance::WhiteBalanceMode,
    ) -> crate::Result<()> {
        self.0.set_white_balance_mode(mode).await
    }

    async fn white_balance_auto(&self) -> crate::Result<()> {
        self.0.white_balance_auto().await
    }

    async fn white_balance_indoor(&self) -> crate::Result<()> {
        self.0.white_balance_indoor().await
    }

    async fn white_balance_outdoor(&self) -> crate::Result<()> {
        self.0.white_balance_outdoor().await
    }

    async fn white_balance_one_push(&self) -> crate::Result<()> {
        self.0.white_balance_one_push().await
    }

    async fn white_balance_atw(&self) -> crate::Result<()> {
        self.0.white_balance_atw().await
    }

    async fn white_balance_manual(&self) -> crate::Result<()> {
        self.0.white_balance_manual().await
    }

    async fn white_balance_color_temperature(&self) -> crate::Result<()> {
        self.0.white_balance_color_temperature().await
    }

    async fn set_awb_sensitivity(
        &self,
        sensitivity: crate::command::white_balance::AutoWhiteBalanceSensitivity,
    ) -> crate::Result<()> {
        self.0.set_awb_sensitivity(sensitivity).await
    }
}

impl<P, T> VariableSpeedOps for Camera<P, T>
where
    P: crate::capabilities::Profile
        + crate::capabilities::VariableSpeed
        + crate::capabilities::HasVariableSpeed,
    T: crate::transport::UnifiedTransport,
{
    async fn set_variable_speed_mode(
        &self,
        mode: crate::command::VariableSpeedMode,
    ) -> crate::Result<()> {
        use crate::camera::methods::variable_speed::VariableSpeedOps as InnerOps;
        InnerOps::set_variable_speed_mode(&self.0, mode).await
    }
}

impl<P, T> ExposureCompensationOps for Camera<P, T>
where
    P: crate::capabilities::Profile
        + crate::capabilities::Exposure
        + crate::capabilities::HasExposureCompensation,
    T: crate::transport::UnifiedTransport,
{
    async fn enable_exposure_compensation(&self) -> crate::Result<()> {
        use crate::camera::methods::exposure::ExposureCompensationOps as InnerOps;
        InnerOps::enable_exposure_compensation(&self.0).await
    }

    async fn disable_exposure_compensation(&self) -> crate::Result<()> {
        use crate::camera::methods::exposure::ExposureCompensationOps as InnerOps;
        InnerOps::disable_exposure_compensation(&self.0).await
    }

    async fn reset_exposure_compensation(&self) -> crate::Result<()> {
        use crate::camera::methods::exposure::ExposureCompensationOps as InnerOps;
        InnerOps::reset_exposure_compensation(&self.0).await
    }

    async fn increase_exposure_compensation(&self) -> crate::Result<()> {
        use crate::camera::methods::exposure::ExposureCompensationOps as InnerOps;
        InnerOps::increase_exposure_compensation(&self.0).await
    }

    async fn decrease_exposure_compensation(&self) -> crate::Result<()> {
        use crate::camera::methods::exposure::ExposureCompensationOps as InnerOps;
        InnerOps::decrease_exposure_compensation(&self.0).await
    }

    async fn set_exposure_compensation_level(&self, level: i8) -> crate::Result<()> {
        use crate::camera::methods::exposure::ExposureCompensationOps as InnerOps;
        InnerOps::set_exposure_compensation_level(&self.0, level).await
    }
}

impl<P: crate::capabilities::Profile, T: crate::transport::UnifiedTransport> StreamingOps
    for Camera<P, T>
{
    async fn enable_multicast(&self) -> crate::Result<()> {
        self.0.enable_multicast().await
    }

    async fn disable_multicast(&self) -> crate::Result<()> {
        self.0.disable_multicast().await
    }

    async fn set_ndi_quality(&self, quality: crate::types::NDIQuality) -> crate::Result<()> {
        self.0.set_ndi_quality(quality).await
    }
}
