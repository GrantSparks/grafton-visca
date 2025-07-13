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

// Implement all blocking traits for the wrapper type using the forward_facade! macro
use crate::forward_facade;

forward_facade!(Camera, blocking,
    ZoomOps:
        zoom_stop() -> crate::Result<()>,
        zoom_in() -> crate::Result<()>,
        zoom_out() -> crate::Result<()>,
        zoom_absolute(position: crate::units::Normalized) -> crate::Result<()>;
);

impl ColorOps for Camera {
    fn one_push_trigger(&self) -> crate::Result<()> {
        self.0.one_push_trigger()
    }

    fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> crate::Result<()> {
        ColorOps::set_color_temperature(&self.0, temp)
    }

    fn color_temperature(&self, temp: Option<crate::types::ColorTemp>) -> crate::Result<()> {
        self.0.color_temperature(temp)
    }

    fn set_red_gain(&self, gain: crate::types::RedChannel) -> crate::Result<()> {
        self.0.set_red_gain(gain)
    }

    fn red_gain(&self, command: crate::command::RedGain) -> crate::Result<()> {
        self.0.red_gain(command)
    }

    fn set_blue_gain(&self, gain: crate::types::BlueChannel) -> crate::Result<()> {
        self.0.set_blue_gain(gain)
    }

    fn blue_gain(&self, command: crate::command::BlueGain) -> crate::Result<()> {
        self.0.blue_gain(command)
    }

    fn set_red_tuning(&self, tuning: crate::types::RedTuning) -> crate::Result<()> {
        self.0.set_red_tuning(tuning)
    }

    fn set_blue_tuning(&self, tuning: crate::types::BlueTuning) -> crate::Result<()> {
        self.0.set_blue_tuning(tuning)
    }
}

impl ExposureOps for Camera {
    fn exposure_auto(&self) -> crate::Result<()> {
        self.0.exposure_auto()
    }

    fn exposure_manual(&self) -> crate::Result<()> {
        self.0.exposure_manual()
    }

    fn set_iris(&self, level: crate::types::IrisLevel) -> crate::Result<()> {
        self.0.set_iris(level)
    }

    fn set_brightness(&self, level: crate::types::BrightnessLevel) -> crate::Result<()> {
        self.0.set_brightness(level)
    }

    fn set_backlight(&self, enabled: bool) -> crate::Result<()> {
        self.0.set_backlight(enabled)
    }

    fn set_gain(&self, gain: crate::types::GainLevel) -> crate::Result<()> {
        self.0.set_gain(gain)
    }

    fn set_gain_limit(&self, limit: crate::types::GainLimit) -> crate::Result<()> {
        self.0.set_gain_limit(limit)
    }

    fn set_dynamic_range(&self, level: crate::types::DynamicRangeLevel) -> crate::Result<()> {
        self.0.set_dynamic_range(level)
    }

    fn set_color_temperature(&self, temp: crate::types::ColorTemp) -> crate::Result<()> {
        ExposureOps::set_color_temperature(&self.0, temp)
    }
}

impl FocusOps for Camera {
    fn focus_auto(&self) -> crate::Result<()> {
        self.0.focus_auto()
    }

    fn focus_manual(&self) -> crate::Result<()> {
        self.0.focus_manual()
    }

    fn focus_near(&self, speed: crate::types::SpeedLevel) -> crate::Result<()> {
        self.0.focus_near(speed)
    }

    fn focus_far(&self, speed: crate::types::SpeedLevel) -> crate::Result<()> {
        self.0.focus_far(speed)
    }

    fn focus_stop(&self) -> crate::Result<()> {
        self.0.focus_stop()
    }

    fn focus_one_push(&self) -> crate::Result<()> {
        self.0.focus_one_push()
    }

    fn set_focus(&self, position: crate::types::FocusPosition) -> crate::Result<()> {
        self.0.set_focus(position)
    }
}

impl ImageProcessingOps for Camera {
    fn enable_flip(&self) -> crate::Result<()> {
        self.0.enable_flip()
    }

    fn set_contrast(&self, level: crate::types::ContrastLevel) -> crate::Result<()> {
        self.0.set_contrast(level)
    }

    fn set_sharpness(&self, level: crate::types::SharpnessLevel) -> crate::Result<()> {
        self.0.set_sharpness(level)
    }

    fn set_saturation(&self, level: crate::types::SaturationLevel) -> crate::Result<()> {
        self.0.set_saturation(level)
    }

    fn set_hue(&self, level: crate::types::HueLevel) -> crate::Result<()> {
        self.0.set_hue(level)
    }

    fn set_noise_reduction_2d(
        &self,
        level: crate::types::NoiseReduction2DLevel,
    ) -> crate::Result<()> {
        self.0.set_noise_reduction_2d(level)
    }

    fn set_noise_reduction_3d(
        &self,
        level: crate::types::NoiseReduction3DLevel,
    ) -> crate::Result<()> {
        self.0.set_noise_reduction_3d(level)
    }

    fn set_image_flip(&self, mode: crate::command::ImageFlipMode) -> crate::Result<()> {
        self.0.set_image_flip(mode)
    }

    fn set_luminance(&self, level: crate::types::LuminanceLevel) -> crate::Result<()> {
        self.0.set_luminance(level)
    }
}

impl InquiryOps for Camera {
    fn get_power_state(&self) -> crate::Result<bool> {
        self.0.get_power_state()
    }

    fn get_zoom_position(&self) -> crate::Result<u16> {
        self.0.get_zoom_position()
    }

    fn get_focus_position(&self) -> crate::Result<u16> {
        self.0.get_focus_position()
    }

    fn get_focus_near_limit(&self) -> crate::Result<u16> {
        self.0.get_focus_near_limit()
    }

    fn get_focus_zone(&self) -> crate::Result<crate::command::FocusZone> {
        self.0.get_focus_zone()
    }

    fn get_auto_focus_sensitivity(&self) -> crate::Result<crate::command::AutoFocusSensitivity> {
        self.0.get_auto_focus_sensitivity()
    }

    fn get_exposure_mode(&self) -> crate::Result<crate::command::ExposureMode> {
        self.0.get_exposure_mode()
    }

    fn get_exposure_compensation(&self) -> crate::Result<i8> {
        self.0.get_exposure_compensation()
    }

    fn get_exposure_compensation_enabled(&self) -> crate::Result<bool> {
        self.0.get_exposure_compensation_enabled()
    }

    fn get_iris(&self) -> crate::Result<u8> {
        self.0.get_iris()
    }

    fn get_shutter(&self) -> crate::Result<u16> {
        self.0.get_shutter()
    }

    fn get_gain(&self) -> crate::Result<u8> {
        self.0.get_gain()
    }

    fn get_gain_limit(&self) -> crate::Result<u8> {
        self.0.get_gain_limit()
    }

    fn get_white_balance_mode(&self) -> crate::Result<crate::command::WhiteBalanceMode> {
        self.0.get_white_balance_mode()
    }

    fn get_red_gain(&self) -> crate::Result<u8> {
        self.0.get_red_gain()
    }

    fn get_blue_gain(&self) -> crate::Result<u8> {
        self.0.get_blue_gain()
    }

    fn get_red_tuning(&self) -> crate::Result<u8> {
        self.0.get_red_tuning()
    }

    fn get_blue_tuning(&self) -> crate::Result<u8> {
        self.0.get_blue_tuning()
    }

    fn get_color_temperature(&self) -> crate::Result<u16> {
        self.0.get_color_temperature()
    }

    fn get_anti_flicker(&self) -> crate::Result<crate::command::AntiFlickerMode> {
        self.0.get_anti_flicker()
    }

    fn get_gamma(&self) -> crate::Result<u8> {
        self.0.get_gamma()
    }

    fn get_contrast(&self) -> crate::Result<u8> {
        self.0.get_contrast()
    }

    fn get_brightness(&self) -> crate::Result<u8> {
        self.0.get_brightness()
    }

    fn get_sharpness(&self) -> crate::Result<u8> {
        self.0.get_sharpness()
    }

    fn get_sharpness_mode(&self) -> crate::Result<crate::command::SharpnessMode> {
        self.0.get_sharpness_mode()
    }

    fn get_saturation(&self) -> crate::Result<u8> {
        self.0.get_saturation()
    }

    fn get_hue(&self) -> crate::Result<u8> {
        self.0.get_hue()
    }

    fn get_noise_reduction_2d(&self) -> crate::Result<u8> {
        self.0.get_noise_reduction_2d()
    }

    fn get_noise_reduction_3d(&self) -> crate::Result<u8> {
        self.0.get_noise_reduction_3d()
    }

    fn get_black_white(&self) -> crate::Result<bool> {
        self.0.get_black_white()
    }
}

impl NDFilterOps for Camera {
    fn set_nd_filter(&self, level: u8) -> crate::Result<()> {
        self.0.set_nd_filter(level)
    }

    fn get_nd_filter(&self) -> crate::Result<u8> {
        self.0.get_nd_filter()
    }
}

impl PanTiltOps for Camera {
    fn pan_tilt_stop(&self) -> crate::Result<()> {
        self.0.pan_tilt_stop()
    }

    fn pan_tilt_home(&self) -> crate::Result<()> {
        self.0.pan_tilt_home()
    }

    fn pan_tilt_absolute(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> crate::Result<()> {
        self.0.pan_tilt_absolute(pan, tilt, speed)
    }

    fn pan_tilt_relative(
        &self,
        pan: crate::units::Degrees,
        tilt: crate::units::Degrees,
        speed: crate::types::SpeedLevel,
    ) -> crate::Result<()> {
        self.0.pan_tilt_relative(pan, tilt, speed)
    }

    fn pan_tilt_move(
        &self,
        direction: crate::command::PanTiltDirection,
        pan_speed: crate::types::PanSpeed,
        tilt_speed: crate::types::TiltSpeed,
    ) -> crate::Result<()> {
        self.0.pan_tilt_move(direction, pan_speed, tilt_speed)
    }

    fn pan_tilt_reset(&self) -> crate::Result<()> {
        self.0.pan_tilt_reset()
    }
}

impl PanTiltInquiryOps for Camera {
    fn get_pan_tilt_position(&self) -> crate::Result<(i16, i16)> {
        self.0.get_pan_tilt_position()
    }

    fn get_pan_tilt_degrees(
        &self,
    ) -> crate::Result<(crate::units::Degrees, crate::units::Degrees)> {
        self.0.get_pan_tilt_degrees()
    }
}

impl PowerOps for Camera {
    fn power_on(&self) -> crate::Result<()> {
        self.0.power_on()
    }

    fn power_off(&self) -> crate::Result<()> {
        self.0.power_off()
    }
}

impl PresetsOps for Camera {
    fn preset_recall(&self, preset: crate::command::PresetNumber) -> crate::Result<()> {
        self.0.preset_recall(preset)
    }

    fn preset_set(&self, preset: crate::command::PresetNumber) -> crate::Result<()> {
        self.0.preset_set(preset)
    }
}

impl SystemOps for Camera {
    fn trigger_address_assignment(&self) -> crate::Result<()> {
        self.0.trigger_address_assignment()
    }

    fn interface_clear(&self) -> crate::Result<()> {
        self.0.interface_clear()
    }

    fn cancel_command(&self, socket: crate::command::Socket) -> crate::Result<()> {
        self.0.cancel_command(socket)
    }
}

impl TallyOps for Camera {
    fn tally_red_on(&self) -> crate::Result<()> {
        self.0.tally_red_on()
    }

    fn tally_red_off(&self) -> crate::Result<()> {
        self.0.tally_red_off()
    }

    fn tally_bright_lo(&self) -> crate::Result<()> {
        self.0.tally_bright_lo()
    }

    fn tally_bright_hi(&self) -> crate::Result<()> {
        self.0.tally_bright_hi()
    }

    fn tally_green_on(&self) -> crate::Result<()> {
        self.0.tally_green_on()
    }

    fn tally_green_off(&self) -> crate::Result<()> {
        self.0.tally_green_off()
    }

    fn tally_flash(&self) -> crate::Result<()> {
        self.0.tally_flash()
    }

    fn tally_on(&self) -> crate::Result<()> {
        self.0.tally_on()
    }

    fn tally_off(&self) -> crate::Result<()> {
        self.0.tally_off()
    }

    fn get_tally_status(&self) -> crate::Result<bool> {
        self.0.get_tally_status()
    }
}

impl WhiteBalanceOps for Camera {
    fn white_balance_auto(&self) -> crate::Result<()> {
        self.0.white_balance_auto()
    }
}
