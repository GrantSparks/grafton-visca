//! Blocking API wrapper for synchronous camera control.
//!
//! This module provides a blocking interface to the camera functionality,
//! exposing only synchronous methods without the `_blocking` suffix.
//!
//! # Example
//!
//! ```ignore
//! use grafton_visca::blocking::{Camera, PowerOps, ZoomOps};
//! use grafton_visca::transport::blocking::Tcp;
//!
//! let transport = Tcp::connect("192.168.1.100:52381")?;
//! let camera = grafton_visca::Camera::new(transport).blocking();
//!
//! // Use clean API without _blocking suffix
//! camera.power_on()?;
//! camera.zoom_in()?;
//! ```

/// A newtype wrapper around the root Camera that exposes only blocking methods.
#[derive(Debug)]
pub struct Camera(pub(super) crate::Camera);

impl Camera {
    /// Create a new blocking camera wrapper.
    pub fn new(inner: crate::Camera) -> Self {
        Self(inner)
    }
}

// Re-export blocking traits with unsuffixed names
pub use crate::camera::methods::{
    ColorOpsBlocking as ColorOps, ExposureOpsBlocking as ExposureOps, FocusOpsBlocking as FocusOps,
    ImageProcessingOpsBlocking as ImageProcessingOps, InquiryOpsBlocking as InquiryOps,
    NDFilterOpsBlocking as NDFilterOps, PanTiltInquiryOpsBlocking as PanTiltInquiryOps,
    PanTiltOpsBlocking as PanTiltOps, PowerOpsBlocking as PowerOps,
    PresetsOpsBlocking as PresetsOps, SystemOpsBlocking as SystemOps, TallyOpsBlocking as TallyOps,
    WhiteBalanceOpsBlocking as WhiteBalanceOps, ZoomOpsBlocking as ZoomOps,
};

// Import traits needed for disambiguation in macros
use crate::camera::methods::{ColorOpsBlocking, ExposureOpsBlocking};

// Implement all blocking traits for the wrapper type using the forward_facade! macro
use crate::forward_facade;

forward_facade!(Camera, blocking,
    ZoomOps:
        zoom_stop() -> crate::Result<()>,
        zoom_in() -> crate::Result<()>,
        zoom_out() -> crate::Result<()>,
        zoom_absolute(position: crate::units::Normalized) -> crate::Result<()>;
    FocusOps:
        focus_auto() -> crate::Result<()>,
        focus_manual() -> crate::Result<()>,
        focus_near(speed: crate::types::SpeedLevel) -> crate::Result<()>,
        focus_far(speed: crate::types::SpeedLevel) -> crate::Result<()>,
        focus_stop() -> crate::Result<()>,
        focus_one_push() -> crate::Result<()>,
        set_focus(position: crate::types::FocusPosition) -> crate::Result<()>;
    PowerOps:
        power_on() -> crate::Result<()>,
        power_off() -> crate::Result<()>;
    PresetsOps:
        preset_recall(preset: crate::command::PresetNumber) -> crate::Result<()>,
        preset_set(preset: crate::command::PresetNumber) -> crate::Result<()>;
    SystemOps:
        trigger_address_assignment() -> crate::Result<()>,
        interface_clear() -> crate::Result<()>,
        cancel_command(socket: crate::command::Socket) -> crate::Result<()>;
    NDFilterOps:
        set_nd_filter(level: u8) -> crate::Result<()>,
        get_nd_filter() -> crate::Result<u8>;
    PanTiltOps:
        pan_tilt_stop() -> crate::Result<()>,
        pan_tilt_home() -> crate::Result<()>,
        pan_tilt_absolute(pan: crate::units::Degrees, tilt: crate::units::Degrees, speed: crate::types::SpeedLevel) -> crate::Result<()>,
        pan_tilt_relative(pan: crate::units::Degrees, tilt: crate::units::Degrees, speed: crate::types::SpeedLevel) -> crate::Result<()>,
        pan_tilt_move(direction: crate::command::PanTiltDirection, pan_speed: crate::types::PanSpeed, tilt_speed: crate::types::TiltSpeed) -> crate::Result<()>,
        pan_tilt_reset() -> crate::Result<()>;
    PanTiltInquiryOps:
        get_pan_tilt_position() -> crate::Result<(i16, i16)>,
        get_pan_tilt_degrees() -> crate::Result<(crate::units::Degrees, crate::units::Degrees)>;
    WhiteBalanceOps:
        white_balance_auto() -> crate::Result<()>;
    ColorOps:
        one_push_trigger() -> crate::Result<()>,
        set_color_temperature@ColorOpsBlocking(temp: crate::types::ColorTemp) -> crate::Result<()>,
        color_temperature(temp: Option<crate::types::ColorTemp>) -> crate::Result<()>,
        set_red_gain(gain: crate::types::RedChannel) -> crate::Result<()>,
        red_gain(command: crate::command::RedGain) -> crate::Result<()>,
        set_blue_gain(gain: crate::types::BlueChannel) -> crate::Result<()>,
        blue_gain(command: crate::command::BlueGain) -> crate::Result<()>,
        set_red_tuning(tuning: crate::types::RedTuning) -> crate::Result<()>,
        set_blue_tuning(tuning: crate::types::BlueTuning) -> crate::Result<()>;
    ExposureOps:
        exposure_auto() -> crate::Result<()>,
        exposure_manual() -> crate::Result<()>,
        set_iris(level: crate::types::IrisLevel) -> crate::Result<()>,
        set_brightness(level: crate::types::BrightnessLevel) -> crate::Result<()>,
        set_backlight(enabled: bool) -> crate::Result<()>,
        set_gain(gain: crate::types::GainLevel) -> crate::Result<()>,
        set_gain_limit(limit: crate::types::GainLimit) -> crate::Result<()>,
        set_dynamic_range(level: crate::types::DynamicRangeLevel) -> crate::Result<()>,
        set_color_temperature@ExposureOpsBlocking(temp: crate::types::ColorTemp) -> crate::Result<()>;
    ImageProcessingOps:
        enable_flip() -> crate::Result<()>,
        set_contrast(level: crate::types::ContrastLevel) -> crate::Result<()>,
        set_sharpness(level: crate::types::SharpnessLevel) -> crate::Result<()>,
        set_saturation(level: crate::types::SaturationLevel) -> crate::Result<()>,
        set_hue(level: crate::types::HueLevel) -> crate::Result<()>,
        set_noise_reduction_2d(level: crate::types::NoiseReduction2DLevel) -> crate::Result<()>,
        set_noise_reduction_3d(level: crate::types::NoiseReduction3DLevel) -> crate::Result<()>,
        set_image_flip(mode: crate::command::ImageFlipMode) -> crate::Result<()>,
        set_luminance(level: crate::types::LuminanceLevel) -> crate::Result<()>;
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
        get_anti_flicker() -> crate::Result<crate::command::AntiFlickerMode>,
        get_gamma() -> crate::Result<u8>,
        get_contrast() -> crate::Result<u8>,
        get_brightness() -> crate::Result<u8>,
        get_sharpness() -> crate::Result<u8>,
        get_sharpness_mode() -> crate::Result<crate::command::SharpnessMode>,
        get_saturation() -> crate::Result<u8>,
        get_hue() -> crate::Result<u8>,
        get_noise_reduction_2d() -> crate::Result<u8>,
        get_noise_reduction_3d() -> crate::Result<u8>,
        get_black_white() -> crate::Result<bool>;
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
        get_tally_status() -> crate::Result<bool>;
);
