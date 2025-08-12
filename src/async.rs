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
//!     let transport = Tcp::connect("192.168.0.110:52381").await?;
//!     let camera = grafton_visca::r#async::Camera::new(grafton_visca::Camera::new(transport));
//!
//!     camera.power_on().await?;
//!     camera.zoom_in().await?;
//!     Ok(())
//! }
//! ```

#[cfg(feature = "async")]
use crate::camera::methods::{ColorOps as CameraColorOps, ExposureOps as CameraExposureOps};
use crate::forward_facade;

/// A newtype wrapper around the root Camera that exposes only async methods.
#[derive(Debug)]
pub struct Camera<P, T>(pub(super) crate::Camera<P, T>)
where
    P: crate::capabilities::Profile,
    T: crate::transport::Transport + Send + Sync + 'static,
    T::Error: Into<crate::Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send;

impl<P, T> Camera<P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::Transport + Send + Sync + 'static,
    T::Error: Into<crate::Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
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

#[cfg(feature = "async")]
pub use crate::camera::methods::{
    ColorOps, DirectMenuControlOps, ExposureCompensationOps, ExposureOps, FocusOps,
    ImageProcessingOps, InquiryOps, MenuControlOps, MotionSyncControl, NDFilterOps,
    PanTiltInquiryOps, PanTiltOps, PowerOps, PresetsOps, StreamingOps, SystemOps, TallyOps,
    VariableSpeedOps, WhiteBalanceOps, ZoomOps,
};

// Use forward_facade macro for all trait implementations
forward_facade!(Camera, async,
    ZoomOps:
        zoom_stop() -> crate::Result<()>,
        zoom_in() -> crate::Result<()>,
        zoom_out() -> crate::Result<()>,
        zoom_in_standard() -> crate::Result<()>,
        zoom_out_standard() -> crate::Result<()>,
        zoom_absolute(position: crate::units::Normalized) -> crate::Result<()>,
        enable_digital_zoom() -> crate::Result<()>,
        disable_digital_zoom() -> crate::Result<()>;
    FocusOps:
        focus_auto() -> crate::Result<()>,
        focus_manual() -> crate::Result<()>,
        focus_near(speed: crate::types::SpeedLevel) -> crate::Result<()>,
        focus_far(speed: crate::types::SpeedLevel) -> crate::Result<()>,
        focus_stop() -> crate::Result<()>,
        focus_one_push() -> crate::Result<()>,
        set_focus(position: crate::types::FocusPosition) -> crate::Result<()>,
        focus_infinity() -> crate::Result<()>,
        enable_focus_lock() -> crate::Result<()>,
        disable_focus_lock() -> crate::Result<()>,
        push_af_press() -> crate::Result<()>,
        push_af_release() -> crate::Result<()>,
        set_focus_zone(zone: crate::command::focus::FocusZone) -> crate::Result<()>,
        set_auto_focus_sensitivity(sensitivity: crate::command::focus::AutoFocusSensitivity) -> crate::Result<()>,
        set_focus_near_limit(position: crate::types::FocusPosition) -> crate::Result<()>;
    PowerOps:
        power_on() -> crate::Result<()>,
        power_off() -> crate::Result<()>;
    PresetsOps:
        preset_recall(preset: crate::command::PresetNumber) -> crate::Result<()>,
        preset_set(preset: crate::command::PresetNumber) -> crate::Result<()>,
        preset_reset(preset: crate::command::PresetNumber) -> crate::Result<()>;
    SystemOps:
        trigger_address_assignment() -> crate::Result<()>,
        interface_clear() -> crate::Result<()>,
        cancel_command(socket: crate::command::system::Socket) -> crate::Result<()>;
    NDFilterOps:
        set_nd_filter_mode(mode: crate::command::NDFilterMode) -> crate::Result<()>,
        set_nd_filter_value(value: u16) -> crate::Result<()>,
        set_nd_filter_stops(stops: f32) -> crate::Result<()>,
        step_nd_filter(direction: crate::command::NDFilterStep) -> crate::Result<()>,
        set_auto_nd(enabled: bool) -> crate::Result<()>,
        get_nd_filter() -> crate::Result<u8>;
    MotionSyncControl:
        set_motion_sync_mode(mode: crate::MotionSyncMode) -> crate::Result<()>,
        set_motion_sync_speed(speed: u8) -> crate::Result<()>,
        set_motion_sync_preset_speed(speed: crate::MotionSyncSpeed) -> crate::Result<()>,
        get_motion_sync_mode() -> crate::Result<crate::MotionSyncMode>,
        get_motion_sync_speed() -> crate::Result<crate::MotionSyncSpeed>;
    MenuControlOps:
        set_menu_display(display: bool) -> crate::Result<()>,
        menu_navigate(direction: crate::command::MenuDirection) -> crate::Result<()>,
        menu_action(action: crate::command::MenuAction) -> crate::Result<()>;
    PanTiltOps:
        pan_tilt_stop() -> crate::Result<()>,
        pan_tilt_home() -> crate::Result<()>,
        pan_tilt_absolute(pan: crate::units::Degrees, tilt: crate::units::Degrees, speed: crate::types::SpeedLevel) -> crate::Result<()>,
        pan_tilt_relative(pan: crate::units::Degrees, tilt: crate::units::Degrees, speed: crate::types::SpeedLevel) -> crate::Result<()>,
        pan_tilt_move(direction: crate::command::PanTiltDirection, pan_speed: crate::types::PanSpeed, tilt_speed: crate::types::TiltSpeed) -> crate::Result<()>,
        pan_tilt_reset() -> crate::Result<()>,
        pan_tilt_limit_set(corner: crate::command::pan_tilt::PanTiltLimitCorner, pan: crate::types::PanPosition, tilt: crate::types::TiltPosition) -> crate::Result<()>,
        pan_tilt_limit_clear(corner: crate::command::pan_tilt::PanTiltLimitCorner) -> crate::Result<()>;
    PanTiltInquiryOps:
        get_pan_tilt_position() -> crate::Result<(i16, i16)>,
        get_pan_tilt_degrees() -> crate::Result<(crate::units::Degrees, crate::units::Degrees)>;
    WhiteBalanceOps:
        set_white_balance_mode(mode: crate::command::white_balance::WhiteBalanceMode) -> crate::Result<()>,
        white_balance_auto() -> crate::Result<()>,
        white_balance_indoor() -> crate::Result<()>,
        white_balance_outdoor() -> crate::Result<()>,
        white_balance_one_push() -> crate::Result<()>,
        white_balance_atw() -> crate::Result<()>,
        white_balance_manual() -> crate::Result<()>,
        white_balance_color_temperature() -> crate::Result<()>,
        set_awb_sensitivity(sensitivity: crate::command::white_balance::AutoWhiteBalanceSensitivity) -> crate::Result<()>;
    ColorOps:
        one_push_trigger() -> crate::Result<()>,
        set_color_temperature@CameraColorOps(temp: crate::types::ColorTemp) -> crate::Result<()>,
        reset_color_temperature() -> crate::Result<()>,
        increase_color_temperature() -> crate::Result<()>,
        decrease_color_temperature() -> crate::Result<()>,
        color_temperature(temp: Option<crate::types::ColorTemp>) -> crate::Result<()>,
        set_red_gain(gain: crate::types::RedChannel) -> crate::Result<()>,
        red_gain(command: crate::command::RedGain) -> crate::Result<()>,
        set_blue_gain(gain: crate::types::BlueChannel) -> crate::Result<()>,
        blue_gain(command: crate::command::BlueGain) -> crate::Result<()>,
        set_red_tuning(tuning: crate::types::RedTuning) -> crate::Result<()>,
        set_blue_tuning(tuning: crate::types::BlueTuning) -> crate::Result<()>;
    ExposureOps:
        set_exposure_mode(mode: crate::command::exposure::ExposureMode) -> crate::Result<()>,
        exposure_auto() -> crate::Result<()>,
        exposure_manual() -> crate::Result<()>,
        exposure_shutter_priority() -> crate::Result<()>,
        exposure_iris_priority() -> crate::Result<()>,
        exposure_bright_mode() -> crate::Result<()>,
        set_iris(level: crate::types::IrisLevel) -> crate::Result<()>,
        reset_iris() -> crate::Result<()>,
        increase_iris() -> crate::Result<()>,
        decrease_iris() -> crate::Result<()>,
        set_brightness(level: crate::types::BrightnessLevel) -> crate::Result<()>,
        reset_brightness() -> crate::Result<()>,
        increase_brightness() -> crate::Result<()>,
        decrease_brightness() -> crate::Result<()>,
        set_backlight(enabled: bool) -> crate::Result<()>,
        set_gain(gain: crate::types::GainLevel) -> crate::Result<()>,
        reset_gain() -> crate::Result<()>,
        increase_gain() -> crate::Result<()>,
        decrease_gain() -> crate::Result<()>,
        set_gain_limit(limit: crate::types::GainLimit) -> crate::Result<()>,
        set_dynamic_range(level: crate::types::DynamicRangeLevel) -> crate::Result<()>,
        set_color_temperature@CameraExposureOps(temp: crate::types::ColorTemp) -> crate::Result<()>,
        set_shutter_speed(speed: crate::types::ShutterSpeed) -> crate::Result<()>,
        reset_shutter_speed() -> crate::Result<()>,
        increase_shutter_speed() -> crate::Result<()>,
        decrease_shutter_speed() -> crate::Result<()>,
        enable_spotlight() -> crate::Result<()>,
        disable_spotlight() -> crate::Result<()>,
        enable_auto_slow_shutter() -> crate::Result<()>,
        disable_auto_slow_shutter() -> crate::Result<()>,
        set_brightness_direct(level: crate::types::BrightnessLevel) -> crate::Result<()>;
    ImageProcessingOps:
        enable_flip() -> crate::Result<()>,
        disable_flip() -> crate::Result<()>,
        enable_horizontal_flip() -> crate::Result<()>,
        disable_horizontal_flip() -> crate::Result<()>,
        set_contrast(level: crate::types::ContrastLevel) -> crate::Result<()>,
        set_sharpness(level: crate::types::SharpnessLevel) -> crate::Result<()>,
        set_sharpness_mode(mode: crate::command::SharpnessMode) -> crate::Result<()>,
        reset_sharpness() -> crate::Result<()>,
        increase_sharpness() -> crate::Result<()>,
        decrease_sharpness() -> crate::Result<()>,
        set_saturation(level: crate::types::SaturationLevel) -> crate::Result<()>,
        set_hue(level: crate::types::HueLevel) -> crate::Result<()>,
        set_noise_reduction_2d(level: crate::types::NoiseReduction2DLevel) -> crate::Result<()>,
        disable_noise_reduction_2d() -> crate::Result<()>,
        set_noise_reduction_3d(level: crate::types::NoiseReduction3DLevel) -> crate::Result<()>,
        disable_noise_reduction_3d() -> crate::Result<()>,
        set_image_flip(mode: crate::command::ImageFlipMode) -> crate::Result<()>,
        set_luminance(level: crate::types::LuminanceLevel) -> crate::Result<()>,
        enable_freeze() -> crate::Result<()>,
        disable_freeze() -> crate::Result<()>,
        enable_black_white() -> crate::Result<()>,
        disable_black_white() -> crate::Result<()>,
        set_picture_effect(mode: crate::PictureEffectMode) -> crate::Result<()>;
    InquiryOps:
        get_power_state() -> crate::Result<bool>,
        get_zoom_position() -> crate::Result<u16>,
        get_focus_position() -> crate::Result<u16>,
        get_focus_near_limit() -> crate::Result<u16>,
        get_focus_zone() -> crate::Result<crate::command::FocusZone>,
        get_auto_focus_sensitivity() -> crate::Result<crate::command::AutoFocusSensitivity>,
        get_exposure_mode() -> crate::Result<crate::command::ExposureMode>,
        get_exposure_compensation() -> crate::Result<i8>,
        get_exposure_compensation_enabled() -> crate::Result<bool>,
        get_iris() -> crate::Result<u8>,
        get_shutter() -> crate::Result<u16>,
        get_gain() -> crate::Result<u8>,
        get_gain_limit() -> crate::Result<u8>,
        get_white_balance_mode() -> crate::Result<crate::command::WhiteBalanceMode>,
        get_red_gain() -> crate::Result<u8>,
        get_blue_gain() -> crate::Result<u8>,
        get_red_tuning() -> crate::Result<u8>,
        get_blue_tuning() -> crate::Result<u8>,
        get_color_temperature() -> crate::Result<u16>,
        get_gamma() -> crate::Result<u8>,
        get_contrast() -> crate::Result<u8>,
        get_brightness() -> crate::Result<u8>,
        get_sharpness() -> crate::Result<u8>,
        get_sharpness_mode() -> crate::Result<crate::command::SharpnessMode>,
        get_saturation() -> crate::Result<u8>,
        get_hue() -> crate::Result<u8>,
        get_noise_reduction_2d() -> crate::Result<u8>,
        get_noise_reduction_3d() -> crate::Result<u8>,
        get_black_white() -> crate::Result<bool>,
        get_resolution() -> crate::Result<crate::command::resolution::ResolutionMode>,
        get_picture_effect() -> crate::Result<crate::command::resolution::PictureEffectMode>,
        get_nd_filter_position() -> crate::Result<crate::command::resolution::NDFilterPosition>,
        get_version() -> crate::Result<crate::command::Version>,
        get_luminance() -> crate::Result<u8>,
        get_backlight_enabled() -> crate::Result<bool>,
        get_image_flip() -> crate::Result<crate::command::ImageFlipStatus>,
        get_dynamic_range() -> crate::Result<u8>,
        get_focus_mode() -> crate::Result<crate::command::FocusMode>,
        get_menu_status() -> crate::Result<bool>,
        get_auto_focus_enabled() -> crate::Result<bool>,
        get_tally_light_status() -> crate::Result<crate::command::TallyStatus>,
        get_night_day_mode() -> crate::Result<crate::command::NightDayMode>,
        get_flip_mode() -> crate::Result<crate::command::FlipMode>,
        get_standby_enabled() -> crate::Result<bool>,
        get_focus_range() -> crate::Result<crate::command::FocusRange>,
        get_iris_control() -> crate::Result<crate::command::IrisControl>,
        get_defog_mode() -> crate::Result<bool>,
        get_defog_level() -> crate::Result<u8>,
        get_digital_ptz_enabled() -> crate::Result<bool>,
        get_auto_white_balance_sensitivity() -> crate::Result<crate::command::AutoWhiteBalanceSensitivity>,
        get_exposure_compensation_position() -> crate::Result<u16>,
        get_auto_trace_enabled() -> crate::Result<bool>,
        get_focus_unlock() -> crate::Result<bool>,
        get_sharpness_position() -> crate::Result<u16>,
        get_noise_reduction_level() -> crate::Result<u8>,
        get_broadcast_domain() -> crate::Result<u8>,
        get_noise_reduction_mode() -> crate::Result<crate::command::NrMode>,
        get_noise_reduction_speed() -> crate::Result<crate::command::NrSpeed>,
        get_black_white_mode() -> crate::Result<crate::command::BlackWhiteMode>,
        get_usb_audio_enabled() -> crate::Result<bool>,
        get_two_tone_mode_enabled() -> crate::Result<bool>,
        get_nd_filter_preset() -> crate::Result<u8>,
        get_digital_mode_enabled() -> crate::Result<bool>,
        get_tally_auto_adjust_enabled() -> crate::Result<bool>,
        get_tally_green_enabled() -> crate::Result<bool>;
    TallyOps:
        tally_red_on() -> crate::Result<()>,
        tally_red_off() -> crate::Result<()>,
        tally_bright_lo() -> crate::Result<()>,
        tally_bright_hi() -> crate::Result<()>,
        tally_green_on() -> crate::Result<()>,
        tally_green_off() -> crate::Result<()>,
        tally_flash() -> crate::Result<()>,
        tally_on() -> crate::Result<()>,
        tally_off() -> crate::Result<()>,
        get_tally_status() -> crate::Result<bool>,
        get_red_tally_status() -> crate::Result<bool>,
        get_green_tally_status() -> crate::Result<bool>;
    StreamingOps:
        enable_multicast() -> crate::Result<()>,
        disable_multicast() -> crate::Result<()>,
        set_ndi_quality(quality: crate::types::NDIQuality) -> crate::Result<()>;
);

// Manual implementations for traits that require special handling
impl<P, T> DirectMenuControlOps for Camera<P, T>
where
    P: crate::capabilities::Profile + crate::capabilities::HasDirectMenuControl,
    T: crate::transport::Transport + Send + Sync + 'static,
    T::Error: Into<crate::Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn direct_menu_control(&self, control1: u8, control2: u8) -> crate::Result<()> {
        self.0.direct_menu_control(control1, control2).await
    }

    async fn toggle_menu(&self) -> crate::Result<()> {
        self.0.toggle_menu().await
    }
}

impl<P, T> VariableSpeedOps for Camera<P, T>
where
    P: crate::capabilities::Profile
        + crate::capabilities::VariableSpeed
        + crate::capabilities::HasVariableSpeed,
    T: crate::transport::Transport + Send + Sync + 'static,
    T::Error: Into<crate::Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
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
    T: crate::transport::Transport + Send + Sync + 'static,
    T::Error: Into<crate::Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
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
