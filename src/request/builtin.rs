//! Homogeneous built-in typed requests.
//!
//! This module owns one typed request for each closed semantic class while the
//! command enums continue to represent wire values. Individual variants are
//! exposed here with one request class each.

use crate::{
    capabilities::TypedSupportSurface,
    command::{
        encode::WireEncode, Flip, Focus, Iris, NdFilterMode, NdFilterStep, NdFilterStepCommand,
        NdFilterValue, PanTilt, PanTiltDirection, PanTiltLimitCorner, PresetAction, PresetCommand,
        PresetNumber, PushAF, Zoom,
    },
    completion, request, AffectedAxes, CameraId, ControlClass, Error, OperationCommand, Request,
    RetryClass, TimeoutClass, ViscaSocket,
};
use std::borrow::Cow;

#[cfg(test)]
std::thread_local! {
    static REQUEST_WRITE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_request_write_count() {
    REQUEST_WRITE_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn request_write_count() -> usize {
    REQUEST_WRITE_COUNT.with(std::cell::Cell::get)
}
use crate::types::{FocusPosition, IrisLevel, PanSpeed, TiltSpeed, ZoomPosition, ZoomSpeed};
use crate::{units::Degrees, PanTiltCoordinateConversion};

macro_rules! impl_request {
    ($type:ty, $class:ty, $size:expr, $timeout:expr, $retry:expr, $control:expr, $wire:expr) => {
        impl Request for $type {
            type Class = $class;

            const MAX_SIZE: usize = $size;
            const TIMEOUT_CLASS: TimeoutClass = $timeout;
            const RETRY_CLASS: RetryClass = $retry;
            const CONTROL_CLASS: ControlClass = $control;

            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                #[cfg(test)]
                REQUEST_WRITE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
                let wire = ($wire)(self);
                WireEncode::write_into(&wire, camera_id, buffer)
            }

            fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
                <Self as BuiltinValidation>::validate(self, profile)
            }

            #[allow(private_interfaces)]
            fn applied_state_projection(
                &self,
                _authority: crate::requests::AppliedStateAuthority,
            ) -> Option<crate::runtime::engine::AppliedStateProjection> {
                <Self as BuiltinValidation>::applied_state(self)
            }
        }
    };
}

pub(crate) trait BuiltinValidation {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error>;

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        None
    }
}

/// Implements the typed plain-request contract directly for one homogeneous
/// built-in command. The command remains the sole wire encoder; this helper
/// supplies only the closed class, policy, profile validation, and applied-state
/// projection selected by the semantic ledger.
macro_rules! impl_plain_request {
    ($type:ty, $size:expr, $timeout:expr, $retry:expr, $control:expr, $encoded_size:expr) => {
        impl Request for $type {
            type Class = request::Plain;

            const MAX_SIZE: usize = $size;
            const TIMEOUT_CLASS: TimeoutClass = $timeout;
            const RETRY_CLASS: RetryClass = $retry;
            const CONTROL_CLASS: ControlClass = $control;

            fn encoded_size(&self) -> usize {
                ($encoded_size)(self)
            }

            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                #[cfg(test)]
                REQUEST_WRITE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
                WireEncode::write_into(self, camera_id, buffer)
            }

            fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
                <Self as BuiltinValidation>::validate(self, profile)
            }

            #[allow(private_interfaces)]
            fn applied_state_projection(
                &self,
                _authority: crate::requests::AppliedStateAuthority,
            ) -> Option<crate::runtime::engine::AppliedStateProjection> {
                <Self as BuiltinValidation>::applied_state(self)
            }
        }
    };
    ($type:ty, $size:expr, $timeout:expr, $retry:expr, $control:expr) => {
        impl Request for $type {
            type Class = request::Plain;

            const MAX_SIZE: usize = $size;
            const TIMEOUT_CLASS: TimeoutClass = $timeout;
            const RETRY_CLASS: RetryClass = $retry;
            const CONTROL_CLASS: ControlClass = $control;

            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                #[cfg(test)]
                REQUEST_WRITE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
                WireEncode::write_into(self, camera_id, buffer)
            }

            fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
                <Self as BuiltinValidation>::validate(self, profile)
            }

            #[allow(private_interfaces)]
            fn applied_state_projection(
                &self,
                _authority: crate::requests::AppliedStateAuthority,
            ) -> Option<crate::runtime::engine::AppliedStateProjection> {
                <Self as BuiltinValidation>::applied_state(self)
            }
        }
    };
}

fn require(supported: bool, feature: &'static str) -> Result<(), Error> {
    if supported {
        Ok(())
    } else {
        Err(Error::FeatureNotSupported { feature })
    }
}

fn invalid_value(parameter: &'static str, value: impl ToString) -> Error {
    Error::InvalidParameter {
        parameter,
        value: Cow::Owned(value.to_string()),
        reason: Cow::Borrowed("value is outside the validated runtime profile"),
    }
}

fn validate_pan_tilt(profile: &crate::ProfileSpec) -> Result<(), Error> {
    require(profile.capabilities().has_pan_tilt, "pan/tilt control")
}

fn validate_pan_tilt_speed(
    profile: &crate::ProfileSpec,
    pan: PanSpeed,
    tilt: TiltSpeed,
) -> Result<(), Error> {
    validate_pan_tilt(profile)?;
    let capabilities = profile.capabilities();
    if !capabilities.pan_speed.contains(&pan.value()) {
        return Err(invalid_value("pan speed", pan.value()));
    }
    if !capabilities.tilt_speed.contains(&tilt.value()) {
        return Err(invalid_value("tilt speed", tilt.value()));
    }
    Ok(())
}

fn validate_pan_tilt_position(
    profile: &crate::ProfileSpec,
    pan: i16,
    tilt: i16,
    conversion: PanTiltCoordinateConversion,
) -> Result<(), Error> {
    validate_pan_tilt(profile)?;
    let capabilities = profile.capabilities();
    if profile.pan_tilt_coordinates() != Some(conversion) {
        return Err(Error::InvalidRequest(
            "pan/tilt request was converted for a different profile".into(),
        ));
    }
    if !capabilities.pan_range.contains(&pan) {
        return Err(invalid_value("pan position", pan));
    }
    if !capabilities.tilt_range.contains(&tilt) {
        return Err(invalid_value("tilt position", tilt));
    }
    Ok(())
}

fn validate_iris_control(profile: &crate::ProfileSpec) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(
        capabilities.has_exposure && capabilities.has_iris_control,
        "iris control",
    )?;
    require(
        capabilities.supports_typed(TypedSupportSurface::IrisControl),
        "typed iris control",
    )
}

fn validate_nd_filter_control(profile: &crate::ProfileSpec) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_nd_filter, "ND filter control")?;
    require(
        capabilities.supports_typed(TypedSupportSurface::NdFilter),
        "typed ND filter control",
    )
}

fn validate_nd_filter_step(profile: &crate::ProfileSpec) -> Result<(), Error> {
    validate_nd_filter_control(profile)?;
    if matches!(
        profile.capabilities().nd_filter_mode,
        crate::capabilities::NdFilterMode::Variable
    ) {
        Ok(())
    } else {
        // The step encoder is documented for the variable-ND VISCA
        // extension.  A generic stepped metadata fact is not evidence that
        // this vendor-specific wire command is valid for that profile.
        Err(Error::FeatureNotSupported {
            feature: "variable ND filter step movement",
        })
    }
}

fn validate_exposure_state(
    profile: &crate::ProfileSpec,
    feature: &'static str,
) -> Result<(), Error> {
    require(profile.capabilities().has_exposure, feature)
}

fn validate_sony_exposure_state(
    profile: &crate::ProfileSpec,
    feature: &'static str,
) -> Result<(), Error> {
    let profile_id = profile.capabilities().profile_id;
    let supported = matches!(
        profile_id,
        Some(crate::camera::profiles::ProfileId::SonyFr7)
            | Some(crate::camera::profiles::ProfileId::SonyBrcH900)
            | Some(crate::camera::profiles::ProfileId::SonyEviH100)
            | Some(crate::camera::profiles::ProfileId::SonyBrc300)
            | Some(crate::camera::profiles::ProfileId::NearusBrc300)
    );
    require(supported, feature)?;
    validate_exposure_state(profile, feature)
}

fn validate_image_state(profile: &crate::ProfileSpec, feature: &'static str) -> Result<(), Error> {
    require(profile.capabilities().has_image_processing, feature)
}

fn validate_tally_state(profile: &crate::ProfileSpec) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_tally, "tally control")?;
    require(
        capabilities.supports_typed(TypedSupportSurface::Tally),
        "typed tally control",
    )
}

fn validate_ptzoptics_vendor_state(
    profile: &crate::ProfileSpec,
    feature: &'static str,
) -> Result<(), Error> {
    let supported = matches!(
        profile.capabilities().profile_id,
        Some(crate::camera::profiles::ProfileId::PtzOpticsG2)
            | Some(crate::camera::profiles::ProfileId::PtzOpticsG3)
            | Some(crate::camera::profiles::ProfileId::PtzOptics30X)
    );
    require(supported, feature)
}

fn validate_power_state(profile: &crate::ProfileSpec, standby: bool) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_power, "power control")?;
    if standby {
        require(capabilities.supports_standby, "standby power control")?;
    }
    Ok(())
}

fn validate_exposure_mode(
    profile: &crate::ProfileSpec,
    mode: crate::command::ExposureMode,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_exposure, "exposure control")?;
    require(
        capabilities.supports_exposure_mode(mode),
        "selected exposure mode",
    )
}

fn validate_exposure_compensation(
    profile: &crate::ProfileSpec,
    value: Option<i8>,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_exposure, "exposure control")?;
    require(
        capabilities.has_exposure_comp
            && capabilities.supports_typed(TypedSupportSurface::ExposureCompensation),
        "exposure compensation",
    )?;
    if let Some(value) = value {
        let range =
            capabilities
                .exposure_comp_range
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "exposure compensation range",
                })?;
        if !range.contains(&value) {
            return Err(invalid_value("exposure compensation", value));
        }
    }
    Ok(())
}

fn validate_numeric_exposure(profile: &crate::ProfileSpec) -> Result<(), Error> {
    require(profile.capabilities().has_exposure, "exposure control")
}

fn validate_shutter(profile: &crate::ProfileSpec, value: Option<u16>) -> Result<(), Error> {
    validate_numeric_exposure(profile)?;
    if let Some(value) = value {
        if !profile
            .capabilities()
            .shutter_speeds
            .iter()
            .any(|speed| speed.value == value)
        {
            return Err(invalid_value("shutter speed", value));
        }
    }
    Ok(())
}

fn validate_brightness(profile: &crate::ProfileSpec, value: Option<u16>) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_exposure, "exposure control")?;
    require(
        capabilities.has_exposure
            && capabilities.supports_typed(TypedSupportSurface::BrightnessControl),
        "brightness control",
    )?;
    if let Some(value) = value {
        let range =
            capabilities
                .exposure_brightness_range
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "brightness range",
                })?;
        if !range.contains(&value) {
            return Err(invalid_value("brightness level", value));
        }
    }
    Ok(())
}

fn validate_gain(profile: &crate::ProfileSpec, value: Option<u8>) -> Result<(), Error> {
    validate_numeric_exposure(profile)?;
    if let Some(value) = value {
        if !profile.capabilities().gain_range.contains(&value) {
            return Err(invalid_value("gain level", value));
        }
    }
    Ok(())
}

fn validate_white_balance_mode(
    profile: &crate::ProfileSpec,
    mode: crate::command::WhiteBalanceMode,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    require(
        capabilities.white_balance_modes.contains(&mode),
        "selected white-balance mode",
    )?;
    match mode {
        crate::command::WhiteBalanceMode::OnePush => require(
            capabilities.has_one_push_wb
                && capabilities.supports_typed(TypedSupportSurface::OnePushWhiteBalance),
            "one-push white balance",
        ),
        crate::command::WhiteBalanceMode::ATW => require(
            capabilities.supports_typed(TypedSupportSurface::AutoTrackingWhiteBalance),
            "auto-tracking white balance",
        ),
        crate::command::WhiteBalanceMode::ColorTemperature => require(
            capabilities.has_color_temp
                && capabilities.supports_typed(TypedSupportSurface::ColorTemperature),
            "color-temperature white balance",
        ),
        _ => Ok(()),
    }
}

fn validate_awb_sensitivity(profile: &crate::ProfileSpec) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    require(
        capabilities.supports_typed(TypedSupportSurface::AutoWhiteBalanceSensitivity),
        "auto white-balance sensitivity",
    )
}

fn validate_tuning(profile: &crate::ProfileSpec, value: i8, red: bool) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    require(
        capabilities.supports_typed(TypedSupportSurface::RgbTuning),
        "RGB tuning",
    )?;
    let range = if red {
        capabilities.rg_tuning_range.as_ref()
    } else {
        capabilities.bg_tuning_range.as_ref()
    }
    .ok_or(Error::FeatureNotSupported {
        feature: "RGB tuning range",
    })?;
    if !range.contains(&value) {
        return Err(invalid_value("RGB tuning", value));
    }
    Ok(())
}

fn validate_color_temperature(
    profile: &crate::ProfileSpec,
    value: Option<u16>,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    require(
        capabilities.has_color_temp
            && capabilities.supports_typed(TypedSupportSurface::ColorTemperature),
        "color temperature control",
    )?;
    if let Some(value) = value {
        let range = capabilities
            .color_temp_range
            .as_ref()
            .ok_or(Error::FeatureNotSupported {
                feature: "color-temperature range",
            })?;
        // ColorTemp values are represented in VISCA steps, while profile
        // metadata is expressed in Kelvin.
        let kelvin = 2_500_u16.saturating_add(value.saturating_mul(100));
        if !range.contains(&kelvin) {
            return Err(invalid_value("color temperature", kelvin));
        }
    }
    Ok(())
}

fn validate_rgb_gain(
    profile: &crate::ProfileSpec,
    value: Option<u8>,
    red: bool,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    require(
        capabilities.has_rgb_gain && capabilities.supports_typed(TypedSupportSurface::RgbGain),
        "RGB gain control",
    )?;
    if let Some(value) = value {
        let range = if red {
            capabilities.red_gain_range.as_ref()
        } else {
            capabilities.blue_gain_range.as_ref()
        }
        .ok_or(Error::FeatureNotSupported {
            feature: "RGB gain range",
        })?;
        if !range.contains(&value) {
            return Err(invalid_value("RGB gain", value));
        }
    }
    Ok(())
}

fn validate_image_control(
    profile: &crate::ProfileSpec,
    surface: TypedSupportSurface,
    feature: &'static str,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_image_processing, "image processing")?;
    require(capabilities.supports_typed(surface), feature)
}

fn validate_image_range(
    profile: &crate::ProfileSpec,
    surface: TypedSupportSurface,
    feature: &'static str,
    range: Option<&std::ops::RangeInclusive<u8>>,
    value: u8,
) -> Result<(), Error> {
    validate_image_control(profile, surface, feature)?;
    let range = range.ok_or(Error::FeatureNotSupported { feature })?;
    if !range.contains(&value) {
        return Err(invalid_value(feature, value));
    }
    Ok(())
}

fn validate_flip_mode(
    profile: &crate::ProfileSpec,
    mode: crate::command::ImageFlipMode,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_image_processing, "image processing")?;
    require(
        capabilities.uses_combined_flip_command
            && capabilities.supports_typed(TypedSupportSurface::CombinedImageFlip),
        "combined image flip",
    )?;
    match mode {
        crate::command::ImageFlipMode::Off | crate::command::ImageFlipMode::Vertical => {
            require(capabilities.supports_flip, "vertical image flip")
        }
        crate::command::ImageFlipMode::Horizontal => {
            require(capabilities.supports_mirror, "horizontal image mirror")
        }
        crate::command::ImageFlipMode::Both => {
            require(capabilities.supports_flip, "vertical image flip")?;
            require(capabilities.supports_mirror, "horizontal image mirror")
        }
    }
}

fn validate_separate_flip(profile: &crate::ProfileSpec, horizontal: bool) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_image_processing, "image processing")?;
    if horizontal {
        require(
            capabilities.supports_mirror
                && capabilities.supports_typed(TypedSupportSurface::ImageMirror),
            "horizontal image mirror",
        )
    } else {
        require(
            capabilities.supports_flip
                && capabilities.supports_typed(TypedSupportSurface::ImageFlip),
            "vertical image flip",
        )
    }
}

fn validate_motion_sync(profile: &crate::ProfileSpec) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_motion_sync, "motion sync")?;
    require(
        capabilities.supports_typed(TypedSupportSurface::MotionSync),
        "typed motion sync",
    )
}

fn validate_motion_sync_speed(profile: &crate::ProfileSpec, speed: u8) -> Result<(), Error> {
    validate_motion_sync(profile)?;
    let maximum =
        profile
            .capabilities()
            .max_motion_sync_speed
            .ok_or(Error::FeatureNotSupported {
                feature: "motion sync speed",
            })?;
    if speed == 0 || speed > maximum {
        return Err(Error::ParameterOutOfRange {
            parameter: "motion sync speed",
            value: speed as i32,
            min: 1,
            max: maximum as i32,
        });
    }
    Ok(())
}

impl BuiltinValidation for crate::command::power::PowerOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_power_state(profile, false)
    }
}

impl BuiltinValidation for crate::command::power::PowerStandby {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_power_state(profile, true)
    }
}

impl BuiltinValidation for crate::command::exposure::ExposureCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_exposure_mode(profile, self.mode)
    }
}

impl BuiltinValidation for crate::command::exposure::ExposureCompensation {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let value = match self {
            Self::SetLevel(level) => Some(level.to_protocol_value() as i8 - 7),
            _ => None,
        };
        validate_exposure_compensation(profile, value)
    }
}

impl BuiltinValidation for crate::command::exposure::DynamicRange {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(profile.capabilities().has_exposure, "exposure control")?;
        require(
            profile.capabilities().has_wdr
                && profile
                    .capabilities()
                    .supports_typed(TypedSupportSurface::WideDynamicRange),
            "wide dynamic range",
        )
    }
}

impl BuiltinValidation for crate::command::exposure::Shutter {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_shutter(
            profile,
            match self {
                Self::SetSpeed(speed) => Some(speed.value()),
                _ => None,
            },
        )
    }
}

impl BuiltinValidation for crate::command::exposure::Brightness {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_brightness(
            profile,
            match self {
                Self::SetLevel(level) | Self::Direct(level) => Some(level.value()),
                _ => None,
            },
        )
    }
}

impl BuiltinValidation for crate::command::exposure::AntiFlickerCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_ptzoptics_vendor_state(profile, "anti-flicker control")
    }
}

impl BuiltinValidation for crate::command::gain::Gain {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_gain(
            profile,
            match self {
                Self::SetValue(level) => Some(level.value()),
                _ => None,
            },
        )
    }
}

impl BuiltinValidation for crate::command::gain::GainLimitCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_gain(profile, Some(self.limit.value()))
    }
}

impl BuiltinValidation for crate::command::color::OnePushTriggerCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_white_balance, "white balance control")?;
        require(
            capabilities.has_one_push_wb
                && capabilities.supports_typed(TypedSupportSurface::OnePushWhiteBalance),
            "one-push white balance",
        )
    }
}

impl BuiltinValidation for crate::command::color::RedTuningCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tuning(profile, self.level.value(), true)
    }
}

impl BuiltinValidation for crate::command::color::BlueTuningCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tuning(profile, self.level.value(), false)
    }
}

impl BuiltinValidation for crate::command::color::SaturationCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            TypedSupportSurface::SaturationControl,
            "saturation control",
            profile.capabilities().saturation_range.as_ref(),
            self.level.value(),
        )
    }
}

impl BuiltinValidation for crate::command::color::HueCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            TypedSupportSurface::HueControl,
            "hue control",
            profile.capabilities().hue_range.as_ref(),
            self.level.value(),
        )
    }
}

impl BuiltinValidation for crate::command::color::ColorTemperature {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_color_temperature(
            profile,
            match self {
                Self::SetTemperature(value) => Some(value.value()),
                _ => None,
            },
        )
    }
}

impl BuiltinValidation for crate::command::color::RedGain {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_rgb_gain(
            profile,
            match self {
                Self::SetValue(value) => Some(value.value()),
                _ => None,
            },
            true,
        )
    }
}

impl BuiltinValidation for crate::command::color::BlueGain {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_rgb_gain(
            profile,
            match self {
                Self::SetValue(value) => Some(value.value()),
                _ => None,
            },
            false,
        )
    }
}

impl BuiltinValidation for crate::command::image::Sharpness {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        match self {
            Self::SetLevel { value } => validate_image_range(
                profile,
                TypedSupportSurface::SharpnessControl,
                "sharpness control",
                profile.capabilities().sharpness_range.as_ref(),
                *value,
            ),
            _ => validate_image_control(
                profile,
                TypedSupportSurface::SharpnessControl,
                "sharpness control",
            ),
        }
    }
}

impl BuiltinValidation for crate::command::image::Luminance {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            TypedSupportSurface::LuminanceControl,
            "luminance control",
            profile.capabilities().luminance_range.as_ref(),
            self.value.value(),
        )
    }
}

impl BuiltinValidation for crate::command::image::Contrast {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            TypedSupportSurface::ContrastControl,
            "contrast control",
            profile.capabilities().contrast_range.as_ref(),
            self.value.value(),
        )
    }
}

impl BuiltinValidation for crate::command::image::GammaCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            TypedSupportSurface::GammaControl,
            "gamma control",
            profile.capabilities().gamma_range.as_ref(),
            self.level.value(),
        )
    }
}

impl BuiltinValidation for crate::command::image::BacklightCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_exposure, "exposure control")?;
        require(
            capabilities.has_backlight_comp
                && capabilities.supports_typed(TypedSupportSurface::BacklightCompensation),
            "backlight compensation",
        )
    }
}

impl BuiltinValidation for crate::command::image::NoiseReduction2D {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_image_processing, "image processing")?;
        require(capabilities.has_2d_nr, "2D noise reduction")?;
        require(
            capabilities.supports_typed(TypedSupportSurface::NoiseReduction2D),
            "typed 2D noise reduction",
        )
    }
}

impl BuiltinValidation for crate::command::image::NoiseReduction3D {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_image_processing, "image processing")?;
        require(capabilities.has_3d_nr, "3D noise reduction")?;
        require(
            capabilities.supports_typed(TypedSupportSurface::NoiseReduction3D),
            "typed 3D noise reduction",
        )
    }
}

impl BuiltinValidation for crate::command::image::ImageFlipCombinedCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_flip_mode(profile, self.mode)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // The combined opcode carries both axes in one parameter byte, so a
        // successful application establishes the complete pair.
        let (horizontal, vertical) = match self.mode {
            crate::command::ImageFlipMode::Off => (false, false),
            crate::command::ImageFlipMode::Horizontal => (true, false),
            crate::command::ImageFlipMode::Vertical => (false, true),
            crate::command::ImageFlipMode::Both => (true, true),
        };
        crate::runtime::engine::AppliedStateProjection::set(
            crate::command::semantics::WriteOnlyState::Flip,
            &[i64::from(horizontal), i64::from(vertical)],
        )
        .ok()
    }
}

impl BuiltinValidation for crate::command::image::PictureEffectCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_control(
            profile,
            TypedSupportSurface::PictureEffect,
            "picture effect",
        )?;
        match self.mode {
            crate::command::PictureEffectMode::Off
            | crate::command::PictureEffectMode::BlackAndWhite
            | crate::command::PictureEffectMode::Unknown(_) => Ok(()),
            _ => Err(Error::FeatureNotSupported {
                feature: "selected picture effect mode",
            }),
        }
    }
}

impl BuiltinValidation for crate::command::focus::FocusZoneCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        require(
            capabilities.has_focus_zone
                && capabilities.supports_typed(TypedSupportSurface::FocusZone),
            "focus zone",
        )
    }
}

impl BuiltinValidation for crate::command::focus::AutoFocusSensitivityCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        require(
            capabilities.has_af_sensitivity
                && capabilities.supports_typed(TypedSupportSurface::AutoFocusSensitivity),
            "auto-focus sensitivity",
        )
    }
}

impl BuiltinValidation for crate::command::focus::FocusNearLimitCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        require(
            capabilities.has_focus_near_limit_inquiry
                && capabilities.supports_typed(TypedSupportSurface::FocusNearLimitInquiry),
            "focus near limit",
        )?;
        let value = self.position.value();
        if capabilities.focus_range.contains(&value) {
            Ok(())
        } else {
            Err(invalid_value("focus near limit", value))
        }
    }
}

impl BuiltinValidation for crate::command::white_balance::WhiteBalanceCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_white_balance_mode(profile, self.mode)
    }
}

impl BuiltinValidation for crate::command::white_balance::AWBSensitivityCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_awb_sensitivity(profile)
    }
}

impl BuiltinValidation for crate::command::menu::SetMenuDisplay {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for crate::command::menu::MenuNavigate {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for crate::command::menu::PerformMenuAction {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for crate::command::menu::DirectMenuControl {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(
            capabilities.has_direct_menu_control
                && capabilities.supports_typed(TypedSupportSurface::DirectMenu),
            "direct menu control",
        )
    }
}

impl BuiltinValidation for crate::command::streaming::UsbAudio {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_ptzoptics_vendor_state(profile, "USB audio control")
    }
}

impl BuiltinValidation for crate::command::system::SettingsSaveCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_ptzoptics_vendor_state(profile, "settings save")
    }
}

impl BuiltinValidation for crate::command::tally::TallyRedOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile)
    }
}

impl BuiltinValidation for crate::command::tally::TallyRedOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile)
    }
}

impl BuiltinValidation for crate::command::tally::TallyGreenOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile)
    }
}

impl BuiltinValidation for crate::command::tally::TallyGreenOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile)
    }
}

impl BuiltinValidation for crate::command::motion_sync::SetMotionSyncMode {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_motion_sync(profile)
    }
}

impl BuiltinValidation for crate::command::motion_sync::SetMotionSyncPreset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_motion_sync_speed(profile, self.speed())
    }
}

impl BuiltinValidation for AddressSet {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for InterfaceClear {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for CommandCancel {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(
            profile.supports_command_cancel(),
            "VISCA socket cancellation",
        )
    }
}

impl BuiltinValidation for ImageFlipCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_separate_flip(profile, false)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // This opcode moves only the vertical axis. The horizontal axis keeps
        // whatever value it had, which this request does not know, so the
        // complete pair stops being known.
        Some(crate::runtime::engine::AppliedStateProjection::invalidate(
            crate::command::semantics::WriteOnlyState::Flip,
        ))
    }
}

impl BuiltinValidation for ImageMirrorCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_separate_flip(profile, true)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // The mirror opcode is the horizontal twin of `ImageFlipCommand` and
        // leaves the vertical axis unknown for the same reason.
        Some(crate::runtime::engine::AppliedStateProjection::invalidate(
            crate::command::semantics::WriteOnlyState::Flip,
        ))
    }
}

fn bool_projection(
    state: crate::command::semantics::WriteOnlyState,
    enabled: bool,
) -> Option<crate::runtime::engine::AppliedStateProjection> {
    crate::runtime::engine::AppliedStateProjection::set(state, &[if enabled { 1 } else { 0 }]).ok()
}

fn scalar_projection(
    state: crate::command::semantics::WriteOnlyState,
    value: i64,
) -> Option<crate::runtime::engine::AppliedStateProjection> {
    crate::runtime::engine::AppliedStateProjection::set(state, &[value]).ok()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreparedPanTiltPosition {
    pan: i16,
    tilt: i16,
    pan_wire: u16,
    tilt_wire: u16,
    conversion: PanTiltCoordinateConversion,
}

impl PreparedPanTiltPosition {
    fn for_profile(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        let conversion = profile
            .pan_tilt_coordinates()
            .ok_or(Error::FeatureNotSupported {
                feature: "pan/tilt coordinate conversion",
            })?;
        let (pan, tilt, pan_wire, tilt_wire) = profile.convert_pan_tilt_degrees(pan.0, tilt.0)?;
        Ok(Self {
            pan,
            tilt,
            pan_wire,
            tilt_wire,
            conversion,
        })
    }

    fn validate(self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_position(profile, self.pan, self.tilt, self.conversion)
    }
}

/// Return pan/tilt to its home position.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PanTiltHome;

impl_request!(
    PanTiltHome,
    request::Operation<completion::Targeted>,
    5,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |_: &PanTiltHome| PanTilt::Home
);

// Plain state commands whose public values are already structurally homogeneous.
// Keeping these implementations on the original command types
// avoids a second public noun for every on/off or mode value.
impl_plain_request!(
    crate::command::preset::PresetRecallSpeedCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::focus::FocusLock,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::exposure::SpotlightOn,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::exposure::SpotlightOff,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::exposure::AutoSlowShutterOn,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::exposure::AutoSlowShutterOff,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::nd_filter::NdFilterModeCommand,
    7,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::nd_filter::AutoNdCommand,
    7,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::flip::ImageFreeze,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::zoom::DigitalZoom,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::streaming::MulticastStreaming,
    6,
    TimeoutClass::Network,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::streaming::SetNdiQuality,
    6,
    TimeoutClass::Network,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::tally::TallyBrightLo,
    8,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::tally::TallyBrightHi,
    8,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::variable_speed::SetVariableSpeedMode,
    7,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::tally::TallyOn,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::tally::TallyOff,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::tally::TallyFlash,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);

// Stateless built-in commands. These values are homogeneous: every
// branch represented by the type is a configuration, mode, persistence, or
// output-policy write.  The request implementation therefore lives directly
// on the public command type and does not need a second noun or a branch
// discriminator.  Mixed physical enums (PanTilt, Zoom, Focus, Iris, ND, and
// Preset) remain represented by the structurally distinct request types below.
impl_plain_request!(
    crate::command::power::PowerOn,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::power::PowerStandby,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::exposure::ExposureCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::exposure::ExposureCompensation,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &crate::command::exposure::ExposureCompensation| match value {
        crate::command::exposure::ExposureCompensation::SetLevel(_) => 9,
        _ => 6,
    }
);
impl_plain_request!(
    crate::command::exposure::DynamicRange,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::exposure::Shutter,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &crate::command::exposure::Shutter| match value {
        crate::command::exposure::Shutter::SetSpeed(_) => 9,
        _ => 6,
    }
);
impl_plain_request!(
    crate::command::exposure::Brightness,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &crate::command::exposure::Brightness| match value {
        crate::command::exposure::Brightness::SetLevel(_)
        | crate::command::exposure::Brightness::Direct(_) => 9,
        _ => 6,
    }
);
impl_plain_request!(
    crate::command::exposure::AntiFlickerCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::gain::Gain,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &crate::command::gain::Gain| match value {
        crate::command::gain::Gain::SetValue(_) => 9,
        _ => 6,
    }
);
impl_plain_request!(
    crate::command::gain::GainLimitCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::color::OnePushTriggerCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::color::RedTuningCommand,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::color::BlueTuningCommand,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::color::SaturationCommand,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::color::HueCommand,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::color::ColorTemperature,
    8,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &crate::command::color::ColorTemperature| match value {
        crate::command::color::ColorTemperature::SetTemperature(_) => 7,
        _ => 6,
    }
);
impl_plain_request!(
    crate::command::color::RedGain,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &crate::command::color::RedGain| match value {
        crate::command::color::RedGain::SetValue(_) => 9,
        _ => 6,
    }
);
impl_plain_request!(
    crate::command::color::BlueGain,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &crate::command::color::BlueGain| match value {
        crate::command::color::BlueGain::SetValue(_) => 9,
        _ => 6,
    }
);
impl_plain_request!(
    crate::command::image::Sharpness,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &crate::command::image::Sharpness| match value {
        crate::command::image::Sharpness::SetLevel { .. } => 9,
        _ => 6,
    }
);
impl_plain_request!(
    crate::command::image::Luminance,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::image::Contrast,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::image::GammaCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::image::BacklightCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::image::NoiseReduction2D,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::image::NoiseReduction3D,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::image::ImageFlipCombinedCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::image::PictureEffectCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::focus::FocusZoneCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::focus::AutoFocusSensitivityCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::focus::FocusNearLimitCommand,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::white_balance::WhiteBalanceCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::white_balance::AWBSensitivityCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::menu::SetMenuDisplay,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::menu::MenuNavigate,
    9,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::menu::PerformMenuAction,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::menu::DirectMenuControl,
    8,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::streaming::UsbAudio,
    7,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::system::SettingsSaveCommand,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::tally::TallyRedOn,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::tally::TallyRedOff,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::tally::TallyGreenOn,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::tally::TallyGreenOff,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::motion_sync::SetMotionSyncMode,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);
impl_plain_request!(
    crate::command::motion_sync::SetMotionSyncPreset,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal
);

/// Broadcast VISCA address-set command.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AddressSet;

impl_request!(
    AddressSet,
    request::Plain,
    4,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |_: &AddressSet| crate::command::system::AddressSetCommand::new()
);

/// Broadcast VISCA interface-clear command.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct InterfaceClear;

impl_request!(
    InterfaceClear,
    request::Plain,
    5,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |_: &InterfaceClear| crate::command::system::InterfaceClearCommand::new()
);

/// Cancel commands on one VISCA socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandCancel {
    socket: ViscaSocket,
}

impl CommandCancel {
    /// Creates a socket-specific cancellation request.
    #[must_use]
    pub const fn new(socket: ViscaSocket) -> Self {
        Self { socket }
    }

    /// Returns the socket selected for cancellation.
    #[must_use]
    pub const fn socket(self) -> ViscaSocket {
        self.socket
    }
}

impl_request!(
    CommandCancel,
    request::Plain,
    3,
    TimeoutClass::Quick,
    RetryClass::Never,
    ControlClass::Urgent,
    |value: &CommandCancel| crate::command::system::CommandCancelCommand::new(value.socket)
);

/// Separate vertical-flip command with its own VISCA opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFlipCommand(pub Flip);

impl ImageFlipCommand {
    /// Creates a vertical-flip request.
    #[must_use]
    pub const fn new(flip: Flip) -> Self {
        Self(flip)
    }
}

impl_request!(
    ImageFlipCommand,
    request::Plain,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &ImageFlipCommand| crate::command::flip::ImageFlip { flip: value.0 }
);

/// Separate horizontal-mirror command with its own VISCA opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageMirrorCommand {
    enabled: bool,
}

impl ImageMirrorCommand {
    /// Creates a horizontal-mirror request.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Returns whether mirroring is enabled.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

impl_request!(
    ImageMirrorCommand,
    request::Plain,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &ImageMirrorCommand| crate::command::flip::HorizontalFlip { on: value.enabled }
);

/// One semantic Plain row tied to a concrete typed request implementation.
///
/// The private marker is a monomorphized `Request<Class = request::Plain>`
/// function pointer.  Keeping that pointer in the inventory means a row can
/// only be added when its named type actually satisfies the closed Plain
/// request contract; the inventory cannot drift into a string-only list.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinTypedRequestCoverage {
    /// Authoritative semantic ledger row.
    pub row: crate::command::semantics::BuiltinCommand,
    /// Stable source type name used by diagnostics and audits.
    pub type_name: &'static str,
    /// Command branch/value family represented by the typed request.
    pub branch: &'static str,
    marker: fn(),
}

#[cfg(test)]
impl BuiltinTypedRequestCoverage {
    /// Returns whether this row is anchored to a Plain request type.
    #[must_use]
    pub fn type_is_plain(self) -> bool {
        let marker = self.marker;
        marker();
        true
    }
}

#[cfg(test)]
fn typed_plain_marker<T: Request<Class = request::Plain>>() {}

#[cfg(test)]
macro_rules! typed_plain_coverage {
    ($row:path, $ty:ty, $branch:literal) => {
        BuiltinTypedRequestCoverage {
            row: $row,
            type_name: stringify!($ty),
            branch: $branch,
            marker: typed_plain_marker::<$ty>,
        }
    };
}

/// Mechanically tied coverage for every semantic Plain row.
///
/// Rows are intentionally repeated when one homogeneous enum represents
/// several protocol branches.  Operation rows are absent by construction and
/// the unit test below checks both uniqueness and exact ledger coverage.
#[cfg(test)]
pub(crate) const BUILTIN_TYPED_REQUEST_INVENTORY: &[BuiltinTypedRequestCoverage] = &[
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::PanTiltLimitSet,
        PanTiltLimitSet,
        "LimitSet"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::PanTiltLimitClear,
        PanTiltLimitClear,
        "LimitClear"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::DigitalZoom,
        crate::command::DigitalZoom,
        "false/true"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::FocusAuto,
        FocusModeCommand,
        "Auto"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::FocusManual,
        FocusModeCommand,
        "Manual"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::FocusToggle,
        FocusModeCommand,
        "Toggle"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::FocusZone,
        crate::command::FocusZoneCommand,
        "Top/Center/Bottom"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::FocusAutoSensitivity,
        crate::command::AutoFocusSensitivityCommand,
        "Low/Normal/High"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::FocusNearLimit,
        crate::command::FocusNearLimitCommand,
        "position"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::FocusLock,
        crate::command::FocusLock,
        "On/Off"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::PresetRecallSpeed,
        crate::command::PresetRecallSpeedCommand,
        "speed"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::PresetSet,
        PresetSet,
        "Set"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::PresetReset,
        PresetReset,
        "Reset"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::PowerOn,
        crate::command::PowerOn,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::PowerStandby,
        crate::command::PowerStandby,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ExposureMode,
        crate::command::ExposureCommand,
        "Auto/Manual/Shutter/Iris/Bright"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ExposureCompensationOn,
        crate::command::ExposureCompensation,
        "On"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ExposureCompensationOff,
        crate::command::ExposureCompensation,
        "Off"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ExposureCompensationReset,
        crate::command::ExposureCompensation,
        "Reset"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ExposureCompensationUp,
        crate::command::ExposureCompensation,
        "Up"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ExposureCompensationDown,
        crate::command::ExposureCompensation,
        "Down"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ExposureCompensationDirect,
        crate::command::ExposureCompensation,
        "SetLevel"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::DynamicRange,
        crate::command::DynamicRange,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ShutterReset,
        crate::command::Shutter,
        "Reset"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ShutterUp,
        crate::command::Shutter,
        "Up"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ShutterDown,
        crate::command::Shutter,
        "Down"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ShutterDirect,
        crate::command::Shutter,
        "SetSpeed"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BrightnessReset,
        crate::command::Brightness,
        "Reset"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BrightnessUp,
        crate::command::Brightness,
        "Up"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BrightnessDown,
        crate::command::Brightness,
        "Down"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BrightnessSet,
        crate::command::Brightness,
        "SetLevel"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BrightnessDirect,
        crate::command::Brightness,
        "Direct"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::AntiFlicker,
        crate::command::AntiFlickerCommand,
        "Off/Hz50/Hz60"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::SpotlightOn,
        crate::command::SpotlightOn,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::SpotlightOff,
        crate::command::SpotlightOff,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::AutoSlowShutterOn,
        crate::command::AutoSlowShutterOn,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::AutoSlowShutterOff,
        crate::command::AutoSlowShutterOff,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::GainReset,
        crate::command::Gain,
        "Reset"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::GainUp,
        crate::command::Gain,
        "Up"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::GainDown,
        crate::command::Gain,
        "Down"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::GainDirect,
        crate::command::Gain,
        "SetValue"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::GainLimit,
        crate::command::GainLimitCommand,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::WhiteBalanceAuto,
        crate::command::WhiteBalanceCommand,
        "Auto"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::WhiteBalanceIndoor,
        crate::command::WhiteBalanceCommand,
        "Indoor"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::WhiteBalanceOutdoor,
        crate::command::WhiteBalanceCommand,
        "Outdoor"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::WhiteBalanceOnePush,
        crate::command::WhiteBalanceCommand,
        "OnePush"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::WhiteBalanceAutoTracking,
        crate::command::WhiteBalanceCommand,
        "ATW"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::WhiteBalanceManual,
        crate::command::WhiteBalanceCommand,
        "Manual"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::WhiteBalanceColorTemperature,
        crate::command::WhiteBalanceCommand,
        "ColorTemperature"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::AutoWhiteBalanceSensitivity,
        crate::command::AWBSensitivityCommand,
        "High/Normal/Low"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::OnePushWhiteBalanceTrigger,
        crate::command::OnePushTriggerCommand,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::RedTuning,
        crate::command::RedTuningCommand,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BlueTuning,
        crate::command::BlueTuningCommand,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::Saturation,
        crate::command::SaturationCommand,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::Hue,
        crate::command::HueCommand,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ColorTemperatureReset,
        crate::command::ColorTemperature,
        "Reset"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ColorTemperatureUp,
        crate::command::ColorTemperature,
        "Up"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ColorTemperatureDown,
        crate::command::ColorTemperature,
        "Down"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ColorTemperatureDirect,
        crate::command::ColorTemperature,
        "SetTemperature"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::RedGainReset,
        crate::command::RedGain,
        "Reset"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::RedGainUp,
        crate::command::RedGain,
        "Up"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::RedGainDown,
        crate::command::RedGain,
        "Down"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::RedGainDirect,
        crate::command::RedGain,
        "SetValue"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BlueGainReset,
        crate::command::BlueGain,
        "Reset"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BlueGainUp,
        crate::command::BlueGain,
        "Up"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BlueGainDown,
        crate::command::BlueGain,
        "Down"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::BlueGainDirect,
        crate::command::BlueGain,
        "SetValue"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::SharpnessMode,
        crate::command::Sharpness,
        "Mode"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::SharpnessReset,
        crate::command::Sharpness,
        "Reset"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::SharpnessUp,
        crate::command::Sharpness,
        "Up"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::SharpnessDown,
        crate::command::Sharpness,
        "Down"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::SharpnessDirect,
        crate::command::Sharpness,
        "SetLevel"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::Luminance,
        crate::command::Luminance,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::Contrast,
        crate::command::Contrast,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::Gamma,
        crate::command::GammaCommand,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::Backlight,
        crate::command::BacklightCommand,
        "false/true"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::NoiseReduction2d,
        crate::command::NoiseReduction2D,
        "None/Some(level)"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::NoiseReduction3d,
        crate::command::NoiseReduction3D,
        "None/Some(level)"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ImageFlipOff,
        ImageFlipCommand,
        "Flip::Off"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ImageFlipVertical,
        ImageFlipCommand,
        "Flip::On"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ImageFlipHorizontal,
        ImageMirrorCommand,
        "true"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ImageFlipHorizontalOff,
        ImageMirrorCommand,
        "false"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ImageFlipBoth,
        crate::command::ImageFlipCombinedCommand,
        "Both"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ImageFlipCombined,
        crate::command::ImageFlipCombinedCommand,
        "encoder"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ImageFreezeOn,
        crate::command::ImageFreeze,
        "true"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::ImageFreezeOff,
        crate::command::ImageFreeze,
        "false"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::PictureEffect,
        crate::command::PictureEffectCommand,
        "mode"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::NdFilterMode,
        crate::command::NdFilterModeCommand,
        "Preset/Variable"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::NdFilterAutoOn,
        crate::command::AutoNdCommand,
        "true"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::NdFilterAutoOff,
        crate::command::AutoNdCommand,
        "false"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::TallyRedOn,
        crate::command::TallyRedOn,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::TallyRedOff,
        crate::command::TallyRedOff,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::TallyBrightLow,
        crate::command::TallyBrightLo,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::TallyBrightHigh,
        crate::command::TallyBrightHi,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::TallyGreenOn,
        crate::command::TallyGreenOn,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::TallyGreenOff,
        crate::command::TallyGreenOff,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::TallyFlash,
        crate::command::TallyFlash,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::TallyOn,
        crate::command::TallyOn,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::TallyOff,
        crate::command::TallyOff,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::MenuDisplay,
        crate::command::SetMenuDisplay,
        "false/true"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::MenuNavigate,
        crate::command::MenuNavigate,
        "Up/Down/Left/Right"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::MenuSelect,
        crate::command::PerformMenuAction,
        "Select"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::MenuCancel,
        crate::command::PerformMenuAction,
        "Cancel"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::DirectMenu,
        crate::command::DirectMenuControl,
        "control1/control2"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::MulticastStreamingOn,
        crate::command::MulticastStreaming,
        "On"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::MulticastStreamingOff,
        crate::command::MulticastStreaming,
        "Off"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::NdiQuality,
        crate::command::SetNdiQuality,
        "High/Medium/Low/Off"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::UsbAudioOn,
        crate::command::UsbAudio,
        "On"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::UsbAudioOff,
        crate::command::UsbAudio,
        "Off"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::AddressSet,
        AddressSet,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::InterfaceClear,
        InterfaceClear,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::CommandCancel,
        CommandCancel,
        "socket"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::SettingsSave,
        crate::command::SettingsSaveCommand,
        "value"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::MotionSyncMode,
        crate::command::SetMotionSyncMode,
        "On/Off"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::MotionSyncPreset,
        crate::command::SetMotionSyncPreset,
        "speed"
    ),
    typed_plain_coverage!(
        crate::command::semantics::BuiltinCommand::VariableSpeedMode,
        crate::command::SetVariableSpeedMode,
        "Standard24/Fine50"
    ),
];

impl OperationCommand<completion::Targeted> for PanTiltHome {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::PAN_TILT
    }
}

/// Reset the pan/tilt mechanism.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PanTiltReset;

impl_request!(
    PanTiltReset,
    request::Operation<completion::Targeted>,
    5,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |_: &PanTiltReset| PanTilt::Reset
);

impl OperationCommand<completion::Targeted> for PanTiltReset {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::PAN_TILT
    }
}

/// Continuous pan/tilt directional drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltDrive {
    direction: PanTiltDirection,
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

impl PanTiltDrive {
    /// Creates a directional drive. STOP has its own structurally distinct type.
    pub fn new(
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<Self, Error> {
        if direction == PanTiltDirection::Stop {
            return Err(Error::InvalidRequest(
                "use PanTiltStop for the stop operation".into(),
            ));
        }
        Ok(Self {
            direction,
            pan_speed,
            tilt_speed,
        })
    }
}

impl_request!(
    PanTiltDrive,
    request::Operation<completion::AppliedOnly>,
    9,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |value: &PanTiltDrive| PanTilt::Move {
        direction: value.direction,
        pan_speed: value.pan_speed,
        tilt_speed: value.tilt_speed,
    }
);

impl OperationCommand<completion::AppliedOnly> for PanTiltDrive {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::PAN_TILT
    }
}

/// Stop pan/tilt movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltStop {
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

impl PanTiltStop {
    /// Creates a stop request with explicit protocol speed fields.
    #[must_use]
    pub const fn new(pan_speed: PanSpeed, tilt_speed: TiltSpeed) -> Self {
        Self {
            pan_speed,
            tilt_speed,
        }
    }
}

impl_request!(
    PanTiltStop,
    request::Operation<completion::AppliedOnly>,
    9,
    TimeoutClass::Quick,
    RetryClass::Movement,
    ControlClass::Urgent,
    |value: &PanTiltStop| PanTilt::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: value.pan_speed,
        tilt_speed: value.tilt_speed,
    }
);

impl OperationCommand<completion::AppliedOnly> for PanTiltStop {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::PAN_TILT
    }
}

/// Absolute pan/tilt target converted from degrees through one validated profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanTiltAbsolute {
    position: PreparedPanTiltPosition,
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

impl PanTiltAbsolute {
    /// Creates an absolute target request.
    pub fn for_profile(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        validate_pan_tilt_speed(profile, pan_speed, tilt_speed)?;
        Ok(Self {
            position: PreparedPanTiltPosition::for_profile(pan, tilt, profile)?,
            pan_speed,
            tilt_speed,
        })
    }
}

impl_request!(
    PanTiltAbsolute,
    request::Operation<completion::Targeted>,
    15,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |value: &PanTiltAbsolute| PanTilt::AbsolutePositionRaw {
        pan_u16: value.position.pan_wire,
        tilt_u16: value.position.tilt_wire,
        pan_speed: value.pan_speed,
        tilt_speed: value.tilt_speed,
    }
);

impl OperationCommand<completion::Targeted> for PanTiltAbsolute {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::PAN_TILT
    }
}

/// Relative pan/tilt offset converted from degrees through one validated profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanTiltRelative {
    position: PreparedPanTiltPosition,
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

impl PanTiltRelative {
    /// Creates a relative target request.
    pub fn for_profile(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        validate_pan_tilt_speed(profile, pan_speed, tilt_speed)?;
        Ok(Self {
            position: PreparedPanTiltPosition::for_profile(pan, tilt, profile)?,
            pan_speed,
            tilt_speed,
        })
    }
}

impl_request!(
    PanTiltRelative,
    request::Operation<completion::Targeted>,
    15,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |value: &PanTiltRelative| PanTilt::RelativePositionRaw {
        pan_u16: value.position.pan_wire,
        tilt_u16: value.position.tilt_wire,
        pan_speed: value.pan_speed,
        tilt_speed: value.tilt_speed,
    }
);

impl OperationCommand<completion::Targeted> for PanTiltRelative {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::PAN_TILT
    }
}

/// Set one pan/tilt movement-limit corner from profile-converted degree values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanTiltLimitSet {
    corner: PanTiltLimitCorner,
    position: PreparedPanTiltPosition,
}

impl PanTiltLimitSet {
    /// Creates a limit-setting command.
    pub fn for_profile(
        corner: PanTiltLimitCorner,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        Ok(Self {
            corner,
            position: PreparedPanTiltPosition::for_profile(pan, tilt, profile)?,
        })
    }
}

impl_request!(
    PanTiltLimitSet,
    request::Plain,
    15,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &PanTiltLimitSet| PanTilt::LimitSetRaw {
        corner: value.corner,
        pan_u16: value.position.pan_wire,
        tilt_u16: value.position.tilt_wire,
    }
);

/// Clear one pan/tilt movement-limit corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltLimitClear {
    corner: PanTiltLimitCorner,
}

impl PanTiltLimitClear {
    /// Creates a limit-clear command.
    #[must_use]
    pub const fn new(corner: PanTiltLimitCorner) -> Self {
        Self { corner }
    }
}

impl_request!(
    PanTiltLimitClear,
    request::Plain,
    15,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &PanTiltLimitClear| PanTilt::LimitClear {
        corner: value.corner,
    }
);

/// Direct zoom target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomTarget(ZoomPosition);

impl ZoomTarget {
    /// Creates a direct zoom target.
    #[must_use]
    pub const fn new(position: ZoomPosition) -> Self {
        Self(position)
    }

    /// Creates a direct zoom target from a normalized position.
    ///
    /// `0.0` is the wide end and `1.0` the telephoto end of the selected
    /// domain. [`ZoomDomain::Optical`] always normalizes across the profile's
    /// documented optical range; [`ZoomDomain::OpticalPlusDigital`] requires a
    /// documented digital maximum and never falls back to the optical one.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected domain has no documented maximum for
    /// `profile`, or when the mapped raw value is outside the VISCA zoom range.
    ///
    /// [`ZoomDomain::Optical`]: crate::ZoomDomain::Optical
    /// [`ZoomDomain::OpticalPlusDigital`]: crate::ZoomDomain::OpticalPlusDigital
    pub fn from_normalized(
        position: crate::units::UnitInterval,
        domain: crate::ZoomDomain,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        let capabilities = profile.capabilities();
        let optical_max = *capabilities.zoom_range_optical.end();
        let digital_max = capabilities
            .zoom_range_digital
            .as_ref()
            .map(|range| *range.end());
        let target = crate::inquiry_conversions::zoom_from_normalized(
            position,
            domain,
            optical_max,
            digital_max,
        )?;
        Ok(Self::new(target))
    }
}

impl_request!(
    ZoomTarget,
    request::Operation<completion::Targeted>,
    9,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |value: &ZoomTarget| Zoom::Position(value.0)
);

impl OperationCommand<completion::Targeted> for ZoomTarget {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::ZOOM
    }
}

/// Continuous zoom-drive direction and speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomDrive {
    /// Drive toward telephoto at standard speed.
    Tele,
    /// Drive toward wide angle at standard speed.
    Wide,
    /// Drive toward telephoto at a variable speed.
    TeleVariable(ZoomSpeed),
    /// Drive toward wide angle at a variable speed.
    WideVariable(ZoomSpeed),
}

impl_request!(
    ZoomDrive,
    request::Operation<completion::AppliedOnly>,
    6,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |value: &ZoomDrive| match value {
        ZoomDrive::Tele => Zoom::TeleStd,
        ZoomDrive::Wide => Zoom::WideStd,
        ZoomDrive::TeleVariable(speed) => Zoom::TeleVariable(*speed),
        ZoomDrive::WideVariable(speed) => Zoom::WideVariable(*speed),
    }
);

impl OperationCommand<completion::AppliedOnly> for ZoomDrive {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::ZOOM
    }
}

/// Stop zoom movement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ZoomStop;

impl_request!(
    ZoomStop,
    request::Operation<completion::AppliedOnly>,
    6,
    TimeoutClass::Quick,
    RetryClass::Movement,
    ControlClass::Urgent,
    |_: &ZoomStop| Zoom::Stop
);

impl OperationCommand<completion::AppliedOnly> for ZoomStop {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::ZOOM
    }
}

/// Direct focus target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusTarget(FocusPosition);

impl FocusTarget {
    /// Creates a direct focus target.
    #[must_use]
    pub const fn new(position: FocusPosition) -> Self {
        Self(position)
    }
}

impl_request!(
    FocusTarget,
    request::Operation<completion::Targeted>,
    9,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |value: &FocusTarget| Focus::Position(value.0)
);

impl OperationCommand<completion::Targeted> for FocusTarget {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::FOCUS
    }
}

/// Move focus to infinity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FocusInfinity;

impl_request!(
    FocusInfinity,
    request::Operation<completion::Targeted>,
    6,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |_: &FocusInfinity| Focus::Infinity
);

impl OperationCommand<completion::Targeted> for FocusInfinity {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::FOCUS
    }
}

/// Continuous focus drive direction and speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDrive {
    /// Drive focus farther at standard speed.
    Far,
    /// Drive focus nearer at standard speed.
    Near,
    /// Drive focus farther at a variable speed.
    FarVariable(crate::command::FocusSpeed),
    /// Drive focus nearer at a variable speed.
    NearVariable(crate::command::FocusSpeed),
}

impl_request!(
    FocusDrive,
    request::Operation<completion::AppliedOnly>,
    6,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |value: &FocusDrive| match value {
        FocusDrive::Far => Focus::Far,
        FocusDrive::Near => Focus::Near,
        FocusDrive::FarVariable(speed) => Focus::FarWithSpeed(*speed),
        FocusDrive::NearVariable(speed) => Focus::NearWithSpeed(*speed),
    }
);

impl OperationCommand<completion::AppliedOnly> for FocusDrive {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::FOCUS
    }
}

/// Stop focus movement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FocusStop;

impl_request!(
    FocusStop,
    request::Operation<completion::AppliedOnly>,
    6,
    TimeoutClass::Quick,
    RetryClass::Movement,
    ControlClass::Urgent,
    |_: &FocusStop| Focus::Stop
);

impl OperationCommand<completion::AppliedOnly> for FocusStop {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::FOCUS
    }
}

/// Instantaneous autofocus trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusTrigger {
    /// Perform one-push autofocus.
    OnePush,
    /// Perform vendor snap focus.
    Snap,
}

impl_request!(
    FocusTrigger,
    request::Operation<completion::AppliedOnly>,
    6,
    TimeoutClass::Quick,
    RetryClass::Movement,
    ControlClass::User,
    |value: &FocusTrigger| match value {
        FocusTrigger::OnePush => Focus::OnePushTrigger,
        FocusTrigger::Snap => Focus::Snap,
    }
);

impl OperationCommand<completion::AppliedOnly> for FocusTrigger {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::FOCUS
    }
}

/// Focus-mode configuration command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusModeCommand {
    /// Enable automatic focus.
    Auto,
    /// Enable manual focus.
    Manual,
    /// Toggle automatic/manual focus on supported cameras.
    Toggle,
}

impl_request!(
    FocusModeCommand,
    request::Plain,
    6,
    TimeoutClass::Quick,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &FocusModeCommand| match value {
        FocusModeCommand::Auto => Focus::Auto,
        FocusModeCommand::Manual => Focus::Manual,
        FocusModeCommand::Toggle => Focus::Toggle,
    }
);

/// Reset the iris/aperture to its camera-defined default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IrisReset;

impl IrisReset {
    /// Creates an iris-reset request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl_request!(
    IrisReset,
    request::Operation<completion::Targeted>,
    6,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |_: &IrisReset| Iris::Reset
);

impl OperationCommand<completion::Targeted> for IrisReset {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::IRIS
    }
}

/// Increase the iris/aperture by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IrisUp;

impl IrisUp {
    /// Creates an iris-step-up request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl_request!(
    IrisUp,
    request::Operation<completion::Targeted>,
    6,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |_: &IrisUp| Iris::Up
);

impl OperationCommand<completion::Targeted> for IrisUp {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::IRIS
    }
}

/// Decrease the iris/aperture by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IrisDown;

impl IrisDown {
    /// Creates an iris-step-down request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl_request!(
    IrisDown,
    request::Operation<completion::Targeted>,
    6,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |_: &IrisDown| Iris::Down
);

impl OperationCommand<completion::Targeted> for IrisDown {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::IRIS
    }
}

/// Set an explicit iris/aperture target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrisDirect(IrisLevel);

impl IrisDirect {
    /// Creates a direct iris target.
    #[must_use]
    pub const fn new(level: IrisLevel) -> Self {
        Self(level)
    }

    /// Returns the requested iris level.
    #[must_use]
    pub const fn level(self) -> IrisLevel {
        self.0
    }
}

impl_request!(
    IrisDirect,
    request::Operation<completion::Targeted>,
    9,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |value: &IrisDirect| Iris::SetAperture(value.0)
);

impl OperationCommand<completion::Targeted> for IrisDirect {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::IRIS
    }
}

/// Set an explicit variable ND-filter target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NdFilterDirect(NdFilterValue);

impl NdFilterDirect {
    /// Creates a direct ND-filter target.
    #[must_use]
    pub const fn new(value: NdFilterValue) -> Self {
        Self(value)
    }

    /// Returns the requested ND-filter value.
    #[must_use]
    pub const fn value(self) -> NdFilterValue {
        self.0
    }
}

impl_request!(
    NdFilterDirect,
    request::Operation<completion::Targeted>,
    9,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |value: &NdFilterDirect| value.0
);

impl OperationCommand<completion::Targeted> for NdFilterDirect {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::ND_FILTER
    }
}

/// Increase the ND filter by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NdFilterStepUp;

impl NdFilterStepUp {
    /// Creates an ND-filter-step-up request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl_request!(
    NdFilterStepUp,
    request::Operation<completion::Targeted>,
    7,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |_: &NdFilterStepUp| NdFilterStepCommand::new(NdFilterStep::Up)
);

impl OperationCommand<completion::Targeted> for NdFilterStepUp {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::ND_FILTER
    }
}

/// Decrease the ND filter by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NdFilterStepDown;

impl NdFilterStepDown {
    /// Creates an ND-filter-step-down request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl_request!(
    NdFilterStepDown,
    request::Operation<completion::Targeted>,
    7,
    TimeoutClass::Movement,
    RetryClass::Movement,
    ControlClass::User,
    |_: &NdFilterStepDown| NdFilterStepCommand::new(NdFilterStep::Down)
);

impl OperationCommand<completion::Targeted> for NdFilterStepDown {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::ND_FILTER
    }
}

/// Press Sony's Push-AF control.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PushAfPress;

impl PushAfPress {
    /// Creates a Push-AF press request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl_request!(
    PushAfPress,
    request::Operation<completion::AppliedOnly>,
    8,
    TimeoutClass::Quick,
    RetryClass::Movement,
    ControlClass::User,
    |_: &PushAfPress| PushAF::Press
);

impl OperationCommand<completion::AppliedOnly> for PushAfPress {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::FOCUS
    }
}

/// Release Sony's Push-AF control.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PushAfRelease;

impl PushAfRelease {
    /// Creates a Push-AF release request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl_request!(
    PushAfRelease,
    request::Operation<completion::AppliedOnly>,
    8,
    TimeoutClass::Quick,
    RetryClass::Movement,
    ControlClass::User,
    |_: &PushAfRelease| PushAF::Release
);

impl OperationCommand<completion::AppliedOnly> for PushAfRelease {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::FOCUS
    }
}

/// Recall a stored preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetRecall {
    preset: PresetNumber,
    axes: AffectedAxes,
}

impl PresetRecall {
    /// Creates a preset recall operation with profile-selected exact axes.
    pub fn for_profile(preset: PresetNumber, profile: &crate::ProfileSpec) -> Result<Self, Error> {
        let axes = profile
            .preset_recall_axes()
            .ok_or(Error::FeatureNotSupported {
                feature: "preset recall",
            })?;
        Ok(Self { preset, axes })
    }
}

impl_request!(
    PresetRecall,
    request::Operation<completion::Targeted>,
    7,
    TimeoutClass::Preset,
    RetryClass::Preset,
    ControlClass::User,
    |value: &PresetRecall| PresetCommand {
        action: PresetAction::Recall,
        preset_number: value.preset,
    }
);

impl OperationCommand<completion::Targeted> for PresetRecall {
    fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }
}

/// Store the current state in a preset slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetSet(PresetNumber);

impl PresetSet {
    /// Creates a preset-store command.
    #[must_use]
    pub const fn new(preset: PresetNumber) -> Self {
        Self(preset)
    }
}

impl_request!(
    PresetSet,
    request::Plain,
    7,
    TimeoutClass::Preset,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &PresetSet| PresetCommand {
        action: PresetAction::Set,
        preset_number: value.0,
    }
);

/// Clear a stored preset slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetReset(PresetNumber);

impl PresetReset {
    /// Creates a preset-clear command.
    #[must_use]
    pub const fn new(preset: PresetNumber) -> Self {
        Self(preset)
    }
}

impl_request!(
    PresetReset,
    request::Plain,
    7,
    TimeoutClass::Preset,
    RetryClass::Standard,
    ControlClass::Normal,
    |value: &PresetReset| PresetCommand {
        action: PresetAction::Reset,
        preset_number: value.0,
    }
);

impl BuiltinValidation for PanTiltHome {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt(profile)
    }
}

impl BuiltinValidation for PanTiltReset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt(profile)
    }
}

impl BuiltinValidation for PanTiltDrive {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_speed(profile, self.pan_speed, self.tilt_speed)
    }
}

impl BuiltinValidation for PanTiltStop {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_speed(profile, self.pan_speed, self.tilt_speed)
    }
}

impl BuiltinValidation for PanTiltAbsolute {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_speed(profile, self.pan_speed, self.tilt_speed)?;
        self.position.validate(profile)
    }
}

impl BuiltinValidation for PanTiltRelative {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_speed(profile, self.pan_speed, self.tilt_speed)?;
        self.position.validate(profile)
    }
}

impl BuiltinValidation for PanTiltLimitSet {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        self.position.validate(profile)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // Keep the corner and converted position in the bounded value so a
        // later owner/cache layer can merge corner-local limit updates without
        // consulting the request or the camera facade.
        crate::runtime::engine::AppliedStateProjection::set(
            crate::command::semantics::WriteOnlyState::PanTiltLimits,
            &[
                i64::from(self.corner.to_byte()),
                i64::from(self.position.pan),
                i64::from(self.position.tilt),
            ],
        )
        .ok()
    }
}

impl BuiltinValidation for PanTiltLimitClear {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt(profile)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        crate::runtime::engine::AppliedStateProjection::clear_with_values(
            crate::command::semantics::WriteOnlyState::PanTiltLimits,
            &[i64::from(self.corner.to_byte())],
        )
        .ok()
    }
}

impl BuiltinValidation for ZoomTarget {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_zoom, "zoom control")?;
        require(
            capabilities.supports_direct_zoom
                && capabilities.supports_typed(TypedSupportSurface::DirectZoom),
            "direct zoom positioning",
        )?;
        let position = self.0.value();
        if capabilities.zoom_range_optical.contains(&position) {
            return Ok(());
        }
        if capabilities
            .zoom_range_digital
            .as_ref()
            .is_some_and(|range| range.contains(&position))
            && capabilities.supports_typed(TypedSupportSurface::DigitalZoomRange)
        {
            Ok(())
        } else {
            Err(invalid_value("zoom position", position))
        }
    }
}

impl BuiltinValidation for ZoomDrive {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_zoom, "zoom control")?;
        let speed = match self {
            Self::Tele | Self::Wide => return Ok(()),
            Self::TeleVariable(speed) | Self::WideVariable(speed) => speed.value(),
        };
        require(capabilities.supports_variable_zoom, "variable zoom drive")?;
        if capabilities.zoom_speed.contains(&speed) {
            Ok(())
        } else {
            Err(invalid_value("zoom speed", speed))
        }
    }
}

impl BuiltinValidation for ZoomStop {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(profile.capabilities().has_zoom, "zoom control")
    }
}

impl BuiltinValidation for FocusTarget {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        if capabilities.focus_range.contains(&self.0.value()) {
            Ok(())
        } else {
            Err(invalid_value("focus position", self.0.value()))
        }
    }
}

impl BuiltinValidation for FocusInfinity {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(profile.capabilities().has_focus, "focus control")
    }
}

impl BuiltinValidation for FocusDrive {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        let speed = match self {
            Self::Far | Self::Near => return Ok(()),
            Self::FarVariable(speed) | Self::NearVariable(speed) => speed.value(),
        };
        if capabilities.focus_speed.contains(&speed) {
            Ok(())
        } else {
            Err(invalid_value("focus speed", speed))
        }
    }
}

impl BuiltinValidation for FocusStop {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(profile.capabilities().has_focus, "focus control")
    }
}

impl BuiltinValidation for FocusTrigger {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        match self {
            Self::OnePush => require(
                capabilities.has_one_push_focus
                    && capabilities.supports_typed(TypedSupportSurface::OnePushFocus),
                "one-push focus",
            ),
            Self::Snap => require(
                capabilities.has_focus
                    && capabilities.supports_typed(TypedSupportSurface::PtzOpticsSnapFocus),
                "snap focus",
            ),
        }
    }
}

impl BuiltinValidation for FocusModeCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        if matches!(self, Self::Auto | Self::Toggle) {
            require(capabilities.has_auto_focus, "automatic focus")?;
        }
        Ok(())
    }
}

impl BuiltinValidation for IrisReset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_iris_control(profile)
    }
}

impl BuiltinValidation for IrisUp {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_iris_control(profile)
    }
}

impl BuiltinValidation for IrisDown {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_iris_control(profile)
    }
}

impl BuiltinValidation for IrisDirect {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_iris_control(profile)?;
        let range =
            profile
                .capabilities()
                .iris_range
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "iris range",
                })?;
        if range.contains(&u16::from(self.0.value())) {
            Ok(())
        } else {
            Err(invalid_value("iris level", self.0.value()))
        }
    }
}

impl BuiltinValidation for NdFilterDirect {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_nd_filter_control(profile)?;
        if !matches!(
            profile.capabilities().nd_filter_mode,
            crate::capabilities::NdFilterMode::Variable
        ) {
            return Err(Error::FeatureNotSupported {
                feature: "direct ND filter positioning",
            });
        }
        // NdFilterValue validates the protocol's exact variable-ND range at
        // construction.  Keep the value extraction here explicit so profile
        // admission cannot silently turn a future wider value into a
        // valid typed request without a corresponding profile fact.
        if self.0.value() <= 0x0014 {
            Ok(())
        } else {
            Err(invalid_value("ND filter value", self.0.value()))
        }
    }
}

impl BuiltinValidation for NdFilterStepUp {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_nd_filter_step(profile)
    }
}

impl BuiltinValidation for NdFilterStepDown {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_nd_filter_step(profile)
    }
}

impl BuiltinValidation for PushAfPress {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        require(
            capabilities.supports_typed(TypedSupportSurface::PushAutoFocus),
            "push autofocus",
        )
    }
}

impl BuiltinValidation for PushAfRelease {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        require(
            capabilities.supports_typed(TypedSupportSurface::PushAutoFocus),
            "push autofocus",
        )
    }
}

fn validate_preset(profile: &crate::ProfileSpec, preset: PresetNumber) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_presets, "preset control")?;
    if preset.value() <= capabilities.max_presets {
        Ok(())
    } else {
        Err(invalid_value("preset number", preset.value()))
    }
}

impl BuiltinValidation for PresetRecall {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.preset)?;
        if profile.preset_recall_axes() == Some(self.axes) {
            Ok(())
        } else {
            Err(Error::InvalidRequest(
                "preset recall was prepared for a different profile axis set".into(),
            ))
        }
    }
}

impl BuiltinValidation for PresetSet {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.0)
    }
}

impl BuiltinValidation for PresetReset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.0)
    }
}

impl BuiltinValidation for crate::command::preset::PresetRecallSpeedCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_ptzoptics_vendor_state(profile, "preset recall speed")?;
        let capabilities = profile.capabilities();
        require(capabilities.has_presets, "preset control")?;
        let speed = self.speed.value();
        if capabilities.preset_speed_range.contains(&speed) {
            Ok(())
        } else {
            Err(invalid_value("preset recall speed", speed))
        }
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        scalar_projection(
            crate::command::semantics::WriteOnlyState::PresetRecallSpeed,
            i64::from(self.speed.value()),
        )
    }
}

impl BuiltinValidation for crate::command::focus::FocusLock {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus lock")?;
        require(
            capabilities.supports_typed(TypedSupportSurface::FocusLock),
            "typed focus lock",
        )
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(
            crate::command::semantics::WriteOnlyState::FocusLockMode,
            matches!(self, Self::On),
        )
    }
}

impl BuiltinValidation for crate::command::exposure::SpotlightOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_sony_exposure_state(profile, "spotlight control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(crate::command::semantics::WriteOnlyState::Spotlight, true)
    }
}

impl BuiltinValidation for crate::command::exposure::SpotlightOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_sony_exposure_state(profile, "spotlight control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(crate::command::semantics::WriteOnlyState::Spotlight, false)
    }
}

impl BuiltinValidation for crate::command::exposure::AutoSlowShutterOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_sony_exposure_state(profile, "auto slow shutter control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(
            crate::command::semantics::WriteOnlyState::AutoSlowShutter,
            true,
        )
    }
}

impl BuiltinValidation for crate::command::exposure::AutoSlowShutterOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_sony_exposure_state(profile, "auto slow shutter control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(
            crate::command::semantics::WriteOnlyState::AutoSlowShutter,
            false,
        )
    }
}

impl BuiltinValidation for crate::command::nd_filter::NdFilterModeCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_nd_filter_control(profile)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        scalar_projection(
            crate::command::semantics::WriteOnlyState::NdFilterMode,
            match self.mode() {
                NdFilterMode::Preset => 0,
                NdFilterMode::Variable => 1,
            },
        )
    }
}

impl BuiltinValidation for crate::command::nd_filter::AutoNdCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_nd_filter_control(profile)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(
            crate::command::semantics::WriteOnlyState::AutoNdFilter,
            self.enabled(),
        )
    }
}

impl BuiltinValidation for crate::command::flip::ImageFreeze {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_state(profile, "image freeze control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(
            crate::command::semantics::WriteOnlyState::ImageFreeze,
            self.enabled(),
        )
    }
}

impl BuiltinValidation for crate::command::zoom::DigitalZoom {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_zoom, "zoom control")?;
        require(capabilities.has_digital_zoom, "digital zoom")?;
        require(
            capabilities.supports_typed(TypedSupportSurface::DigitalZoomToggle),
            "typed digital zoom toggle",
        )
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(
            crate::command::semantics::WriteOnlyState::DigitalZoomMode,
            self.enabled(),
        )
    }
}

impl BuiltinValidation for crate::command::streaming::MulticastStreaming {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_ptzoptics_vendor_state(profile, "multicast streaming")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(
            crate::command::semantics::WriteOnlyState::MulticastStreaming,
            matches!(self, Self::On),
        )
    }
}

impl BuiltinValidation for crate::command::streaming::SetNdiQuality {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_ptzoptics_vendor_state(profile, "NDI quality control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        let value = match self.quality {
            crate::types::NdiQuality::High => 1,
            crate::types::NdiQuality::Medium => 2,
            crate::types::NdiQuality::Low => 3,
            crate::types::NdiQuality::Off => 4,
        };
        scalar_projection(crate::command::semantics::WriteOnlyState::NdiQuality, value)
    }
}

impl BuiltinValidation for crate::command::tally::TallyBrightLo {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        scalar_projection(
            crate::command::semantics::WriteOnlyState::TallyBrightness,
            0,
        )
    }
}

impl BuiltinValidation for crate::command::tally::TallyBrightHi {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        scalar_projection(
            crate::command::semantics::WriteOnlyState::TallyBrightness,
            1,
        )
    }
}

impl BuiltinValidation for crate::command::variable_speed::SetVariableSpeedMode {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_pan_tilt, "pan/tilt control")?;
        require(capabilities.has_variable_speed, "variable speed mode")?;
        require(
            capabilities.supports_typed(TypedSupportSurface::VariableSpeed),
            "typed variable speed mode",
        )
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        let value = match self.mode {
            crate::command::VariableSpeedMode::Standard24 => 1,
            crate::command::VariableSpeedMode::Fine50 => 2,
        };
        scalar_projection(
            crate::command::semantics::WriteOnlyState::VariableSpeedMode,
            value,
        )
    }
}

impl BuiltinValidation for crate::command::tally::TallyOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_ptzoptics_vendor_state(profile, "tally mode")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(crate::command::semantics::WriteOnlyState::TallyMode, true)
    }
}

impl BuiltinValidation for crate::command::tally::TallyOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_ptzoptics_vendor_state(profile, "tally mode")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection(crate::command::semantics::WriteOnlyState::TallyMode, false)
    }
}

impl BuiltinValidation for crate::command::tally::TallyFlash {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_ptzoptics_vendor_state(profile, "tally mode")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        Some(crate::runtime::engine::AppliedStateProjection::invalidate(
            crate::command::semantics::WriteOnlyState::TallyMode,
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        command::{
            AutoNdCommand, AutoSlowShutterOff, AutoSlowShutterOn, DigitalZoom, FocusLock,
            ImageFreeze, MulticastStreaming, NdFilterMode, NdFilterModeCommand, PanTiltLimitCorner,
            PresetRecallSpeed, SetNdiQuality, SpotlightOff, SpotlightOn, TallyBrightHi,
            TallyBrightLo, TallyFlash, TallyOff, TallyOn, VariableSpeedMode,
        },
        prepared::{
            prepare_builtin_command, prepare_builtin_operation, prepare_command, ClassSelection,
        },
        profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        types::NdiQuality,
        OperationalTuning, PlainCommand, ProfileSpec,
    };
    use std::collections::HashSet;

    fn wire<R: Request>(request: &R) -> Vec<u8> {
        let mut buffer = [0_u8; 32];
        let length = request
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("typed request must encode");
        buffer[..length].to_vec()
    }

    fn command_wire<C: WireEncode>(command: &C) -> Vec<u8> {
        let mut buffer = [0_u8; 32];
        let length = command
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("command must encode");
        buffer[..length].to_vec()
    }

    fn prepared_state<C>(
        command: &C,
        profile: &ProfileSpec,
    ) -> crate::runtime::engine::AppliedStateProjection
    where
        C: PlainCommand + ?Sized,
    {
        prepare_command(
            command,
            CameraId::CAMERA_1,
            profile,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("state command must prepare")
        .admit_with(|request, _timeout| match request {
            crate::runtime::engine::RuntimeRequest::Command {
                applied_state: Some(state),
                ..
            } => state,
            crate::runtime::engine::RuntimeRequest::Command {
                applied_state: None,
                ..
            } => panic!("state command prepared without an applied-state projection"),
            crate::runtime::engine::RuntimeRequest::Inquiry { .. } => {
                panic!("plain state command prepared as an inquiry")
            }
        })
    }

    fn assert_state_row<C>(
        name: &str,
        command: &C,
        profile: &ProfileSpec,
        semantic: crate::command::semantics::BuiltinCommand,
        requirement: crate::command::semantics::AppliedStateEffectRequirement,
        expected: crate::runtime::engine::AppliedStateProjection,
    ) where
        C: PlainCommand + ?Sized,
    {
        assert_eq!(
            semantic.classification(),
            crate::command::semantics::BuiltinRequestClass::Plain {
                state_effect: Some(requirement),
            },
            "{name} classification"
        );
        let actual = prepared_state(command, profile);
        assert_eq!(actual, expected, "{name} applied-state projection");
    }

    #[test]
    fn every_write_only_state_command_has_an_exact_closed_projection() {
        use crate::command::semantics::{
            AppliedStateEffectRequirement::{Clear, Invalidate, Set},
            BuiltinCommand as B, WriteOnlyState as S,
        };

        let ptz = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZ profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");

        macro_rules! row {
            ($name:literal, $command:expr, $profile:expr, $semantic:expr, $requirement:expr, $expected:expr) => {{
                let command = $command;
                assert_state_row(
                    $name,
                    &command,
                    $profile,
                    $semantic,
                    $requirement,
                    $expected,
                );
            }};
        }

        // Pan/tilt limits carry the corner discriminator in both actions. A
        // set additionally carries the two converted coordinates, staying
        // within the four-scalar projection bound.
        row!(
            "pan/tilt limit set",
            PanTiltLimitSet::for_profile(
                PanTiltLimitCorner::DownLeft,
                Degrees(0.0),
                Degrees(0.0),
                &ptz,
            )
            .expect("limit set"),
            &ptz,
            B::PanTiltLimitSet,
            Set(S::PanTiltLimits),
            crate::runtime::engine::AppliedStateProjection::set(
                S::PanTiltLimits,
                &[i64::from(PanTiltLimitCorner::DownLeft.to_byte()), 0, 0],
            )
            .expect("limit set projection")
        );
        row!(
            "pan/tilt limit clear",
            PanTiltLimitClear::new(PanTiltLimitCorner::UpRight),
            &ptz,
            B::PanTiltLimitClear,
            Clear(S::PanTiltLimits),
            crate::runtime::engine::AppliedStateProjection::clear_with_values(
                S::PanTiltLimits,
                &[i64::from(PanTiltLimitCorner::UpRight.to_byte())],
            )
            .expect("limit clear projection")
        );

        row!(
            "preset recall speed",
            crate::command::PresetRecallSpeedCommand {
                speed: PresetRecallSpeed::new(12).expect("preset speed"),
            },
            &ptz,
            B::PresetRecallSpeed,
            Set(S::PresetRecallSpeed),
            crate::runtime::engine::AppliedStateProjection::set(S::PresetRecallSpeed, &[12])
                .expect("preset speed projection")
        );
        row!(
            "focus lock on",
            FocusLock::On,
            &ptz,
            B::FocusLock,
            Set(S::FocusLockMode),
            crate::runtime::engine::AppliedStateProjection::set(S::FocusLockMode, &[1])
                .expect("focus lock projection")
        );
        row!(
            "focus lock off",
            FocusLock::Off,
            &ptz,
            B::FocusLock,
            Set(S::FocusLockMode),
            crate::runtime::engine::AppliedStateProjection::set(S::FocusLockMode, &[0])
                .expect("focus lock projection")
        );

        row!(
            "spotlight on",
            SpotlightOn::new(),
            &fr7,
            B::SpotlightOn,
            Set(S::Spotlight),
            crate::runtime::engine::AppliedStateProjection::set(S::Spotlight, &[1])
                .expect("spotlight projection")
        );
        row!(
            "spotlight off",
            SpotlightOff::new(),
            &fr7,
            B::SpotlightOff,
            Set(S::Spotlight),
            crate::runtime::engine::AppliedStateProjection::set(S::Spotlight, &[0])
                .expect("spotlight projection")
        );
        row!(
            "auto slow shutter on",
            AutoSlowShutterOn::new(),
            &fr7,
            B::AutoSlowShutterOn,
            Set(S::AutoSlowShutter),
            crate::runtime::engine::AppliedStateProjection::set(S::AutoSlowShutter, &[1])
                .expect("slow shutter projection")
        );
        row!(
            "auto slow shutter off",
            AutoSlowShutterOff::new(),
            &fr7,
            B::AutoSlowShutterOff,
            Set(S::AutoSlowShutter),
            crate::runtime::engine::AppliedStateProjection::set(S::AutoSlowShutter, &[0])
                .expect("slow shutter projection")
        );

        row!(
            "ND filter preset mode",
            NdFilterModeCommand::new(NdFilterMode::Preset),
            &fr7,
            B::NdFilterMode,
            Set(S::NdFilterMode),
            crate::runtime::engine::AppliedStateProjection::set(S::NdFilterMode, &[0])
                .expect("ND mode projection")
        );
        row!(
            "ND filter variable mode",
            NdFilterModeCommand::new(NdFilterMode::Variable),
            &fr7,
            B::NdFilterMode,
            Set(S::NdFilterMode),
            crate::runtime::engine::AppliedStateProjection::set(S::NdFilterMode, &[1])
                .expect("ND mode projection")
        );
        row!(
            "automatic ND on",
            AutoNdCommand::new(true),
            &fr7,
            B::NdFilterAutoOn,
            Set(S::AutoNdFilter),
            crate::runtime::engine::AppliedStateProjection::set(S::AutoNdFilter, &[1])
                .expect("automatic ND projection")
        );
        row!(
            "automatic ND off",
            AutoNdCommand::new(false),
            &fr7,
            B::NdFilterAutoOff,
            Set(S::AutoNdFilter),
            crate::runtime::engine::AppliedStateProjection::set(S::AutoNdFilter, &[0])
                .expect("automatic ND projection")
        );

        row!(
            "image freeze on",
            ImageFreeze::on(),
            &fr7,
            B::ImageFreezeOn,
            Set(S::ImageFreeze),
            crate::runtime::engine::AppliedStateProjection::set(S::ImageFreeze, &[1])
                .expect("image freeze projection")
        );
        row!(
            "image freeze off",
            ImageFreeze::off(),
            &fr7,
            B::ImageFreezeOff,
            Set(S::ImageFreeze),
            crate::runtime::engine::AppliedStateProjection::set(S::ImageFreeze, &[0])
                .expect("image freeze projection")
        );
        row!(
            "digital zoom on",
            DigitalZoom::new(true),
            &fr7,
            B::DigitalZoom,
            Set(S::DigitalZoomMode),
            crate::runtime::engine::AppliedStateProjection::set(S::DigitalZoomMode, &[1])
                .expect("digital zoom projection")
        );
        row!(
            "digital zoom off",
            DigitalZoom::new(false),
            &fr7,
            B::DigitalZoom,
            Set(S::DigitalZoomMode),
            crate::runtime::engine::AppliedStateProjection::set(S::DigitalZoomMode, &[0])
                .expect("digital zoom projection")
        );

        row!(
            "multicast on",
            MulticastStreaming::On,
            &ptz,
            B::MulticastStreamingOn,
            Set(S::MulticastStreaming),
            crate::runtime::engine::AppliedStateProjection::set(S::MulticastStreaming, &[1])
                .expect("multicast projection")
        );
        row!(
            "multicast off",
            MulticastStreaming::Off,
            &ptz,
            B::MulticastStreamingOff,
            Set(S::MulticastStreaming),
            crate::runtime::engine::AppliedStateProjection::set(S::MulticastStreaming, &[0])
                .expect("multicast projection")
        );
        for (name, quality, value) in [
            ("high", NdiQuality::High, 1),
            ("medium", NdiQuality::Medium, 2),
            ("low", NdiQuality::Low, 3),
            ("off", NdiQuality::Off, 4),
        ] {
            let command = SetNdiQuality::new(quality);
            assert_state_row(
                &format!("NDI quality {name}"),
                &command,
                &ptz,
                B::NdiQuality,
                Set(S::NdiQuality),
                crate::runtime::engine::AppliedStateProjection::set(S::NdiQuality, &[value])
                    .expect("NDI quality projection"),
            );
        }

        row!(
            "tally brightness low",
            TallyBrightLo::new(),
            &fr7,
            B::TallyBrightLow,
            Set(S::TallyBrightness),
            crate::runtime::engine::AppliedStateProjection::set(S::TallyBrightness, &[0])
                .expect("tally brightness projection")
        );
        row!(
            "tally brightness high",
            TallyBrightHi::new(),
            &fr7,
            B::TallyBrightHigh,
            Set(S::TallyBrightness),
            crate::runtime::engine::AppliedStateProjection::set(S::TallyBrightness, &[1])
                .expect("tally brightness projection")
        );
        row!(
            "variable speed standard",
            crate::command::SetVariableSpeedMode::new(VariableSpeedMode::Standard24),
            &fr7,
            B::VariableSpeedMode,
            Set(S::VariableSpeedMode),
            crate::runtime::engine::AppliedStateProjection::set(S::VariableSpeedMode, &[1])
                .expect("variable speed projection")
        );
        row!(
            "variable speed fine",
            crate::command::SetVariableSpeedMode::new(VariableSpeedMode::Fine50),
            &fr7,
            B::VariableSpeedMode,
            Set(S::VariableSpeedMode),
            crate::runtime::engine::AppliedStateProjection::set(S::VariableSpeedMode, &[2])
                .expect("variable speed projection")
        );

        row!(
            "tally on",
            TallyOn::new(),
            &ptz,
            B::TallyOn,
            Set(S::TallyMode),
            crate::runtime::engine::AppliedStateProjection::set(S::TallyMode, &[1])
                .expect("tally mode projection")
        );
        row!(
            "tally off",
            TallyOff::new(),
            &ptz,
            B::TallyOff,
            Set(S::TallyMode),
            crate::runtime::engine::AppliedStateProjection::set(S::TallyMode, &[0])
                .expect("tally mode projection")
        );
        row!(
            "tally flash",
            TallyFlash::new(),
            &ptz,
            B::TallyFlash,
            Invalidate(S::TallyMode),
            crate::runtime::engine::AppliedStateProjection::invalidate(S::TallyMode)
        );

        // The combined-flip opcode carries both axes, so it records the pair
        // as `[horizontal, vertical]`. The single-axis opcodes move one axis
        // and leave the other unknown, so they invalidate the same key.
        for (name, mode, horizontal, vertical) in [
            (
                "combined flip off",
                crate::command::ImageFlipMode::Off,
                0,
                0,
            ),
            (
                "combined flip horizontal",
                crate::command::ImageFlipMode::Horizontal,
                1,
                0,
            ),
            (
                "combined flip vertical",
                crate::command::ImageFlipMode::Vertical,
                0,
                1,
            ),
            (
                "combined flip both",
                crate::command::ImageFlipMode::Both,
                1,
                1,
            ),
        ] {
            assert_state_row(
                name,
                &crate::command::ImageFlipCombinedCommand::new(mode),
                &ptz,
                B::ImageFlipCombined,
                Set(S::Flip),
                crate::runtime::engine::AppliedStateProjection::set(
                    S::Flip,
                    &[horizontal, vertical],
                )
                .expect("combined flip projection"),
            );
        }
        row!(
            "separate vertical flip",
            ImageFlipCommand::new(Flip::On),
            &ptz,
            B::ImageFlipVertical,
            Invalidate(S::Flip),
            crate::runtime::engine::AppliedStateProjection::invalidate(S::Flip)
        );
        row!(
            "separate horizontal mirror",
            ImageMirrorCommand::new(true),
            &ptz,
            B::ImageFlipHorizontal,
            Invalidate(S::Flip),
            crate::runtime::engine::AppliedStateProjection::invalidate(S::Flip)
        );
        row!(
            "separate horizontal mirror off",
            ImageMirrorCommand::new(false),
            &ptz,
            B::ImageFlipHorizontalOff,
            Invalidate(S::Flip),
            crate::runtime::engine::AppliedStateProjection::invalidate(S::Flip)
        );
    }

    #[test]
    fn unsupported_write_only_state_commands_fail_before_encoding() {
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");

        reset_request_write_count();
        assert!(prepare_builtin_command(
            &crate::command::PresetRecallSpeedCommand {
                speed: PresetRecallSpeed::new(12).expect("preset speed"),
            },
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &FocusLock::On,
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &SpotlightOn::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &AutoSlowShutterOn::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &NdFilterModeCommand::new(NdFilterMode::Variable),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &DigitalZoom::new(true),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &MulticastStreaming::On,
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &SetNdiQuality::new(NdiQuality::High),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &TallyBrightLo::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &crate::command::SetVariableSpeedMode::new(VariableSpeedMode::Standard24),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &TallyOn::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &TallyFlash::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert_eq!(request_write_count(), 0);
    }

    #[test]
    fn write_only_state_requests_use_their_exact_plain_policies() {
        assert_policy::<PanTiltLimitSet>(
            15,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<PanTiltLimitClear>(
            15,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<crate::command::PresetRecallSpeedCommand>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<FocusLock>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<SpotlightOn>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<SpotlightOff>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<AutoSlowShutterOn>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<AutoSlowShutterOff>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<NdFilterModeCommand>(
            7,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<AutoNdCommand>(
            7,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<ImageFreeze>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<DigitalZoom>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<MulticastStreaming>(
            6,
            TimeoutClass::Network,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<SetNdiQuality>(
            6,
            TimeoutClass::Network,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyBrightLo>(
            8,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyBrightHi>(
            8,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<crate::command::SetVariableSpeedMode>(
            7,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyOn>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyOff>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyFlash>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
    }

    fn assert_targeted<R>()
    where
        R: Request<Class = request::Operation<completion::Targeted>>
            + OperationCommand<completion::Targeted>,
    {
        let _ = core::marker::PhantomData::<R>;
    }

    fn assert_applied_only<R>()
    where
        R: Request<Class = request::Operation<completion::AppliedOnly>>
            + OperationCommand<completion::AppliedOnly>,
    {
        let _ = core::marker::PhantomData::<R>;
    }

    fn assert_policy<R: Request>(
        max_size: usize,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) {
        assert_eq!(R::MAX_SIZE, max_size);
        assert_eq!(R::TIMEOUT_CLASS, timeout);
        assert_eq!(R::RETRY_CLASS, retry);
        assert_eq!(R::CONTROL_CLASS, control);
    }

    fn assert_exact_request_size<R: Request>(request: &R) {
        let mut buffer = [0_u8; 32];
        let written = request
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("request must encode");
        assert_eq!(written, request.encoded_size());
        assert!(written <= R::MAX_SIZE);
    }

    #[test]
    fn plain_command_values_report_exact_wire_sizes() {
        assert_exact_request_size(&crate::command::ExposureCompensation::On);
        assert_exact_request_size(&crate::command::ExposureCompensation::SetLevel(
            crate::types::ExposureCompensationLevel::new(0).expect("valid compensation"),
        ));
        assert_exact_request_size(&crate::command::Shutter::Reset);
        assert_exact_request_size(&crate::command::Shutter::SetSpeed(
            crate::types::ShutterSpeed::new(1).expect("valid shutter"),
        ));
        assert_exact_request_size(&crate::command::Brightness::Reset);
        assert_exact_request_size(&crate::command::Brightness::SetLevel(
            crate::types::BrightnessLevel::new(1).expect("valid brightness"),
        ));
        assert_exact_request_size(&crate::command::Gain::Reset);
        assert_exact_request_size(&crate::command::Gain::SetValue(
            crate::types::GainLevel::new(1).expect("valid gain"),
        ));
        assert_exact_request_size(&crate::command::ColorTemperature::Reset);
        assert_exact_request_size(&crate::command::ColorTemperature::SetTemperature(
            crate::types::ColorTemp::new(0x20).expect("valid color temperature"),
        ));
        assert_exact_request_size(&crate::command::RedGain::Reset);
        assert_exact_request_size(&crate::command::RedGain::SetValue(
            crate::types::RedChannel::new(1).expect("valid red gain"),
        ));
        assert_exact_request_size(&crate::command::BlueGain::Reset);
        assert_exact_request_size(&crate::command::BlueGain::SetValue(
            crate::types::BlueChannel::new(1).expect("valid blue gain"),
        ));
        assert_exact_request_size(&crate::command::Sharpness::Reset);
        assert_exact_request_size(&crate::command::Sharpness::SetLevel { value: 1 });
    }

    #[test]
    fn focus_mode_commands_do_not_project_write_only_state() {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
        for command in [
            FocusModeCommand::Auto,
            FocusModeCommand::Manual,
            FocusModeCommand::Toggle,
        ] {
            let request = prepare_builtin_command(
                &command,
                CameraId::CAMERA_1,
                &profile,
                OperationalTuning::new(),
            )
            .expect("focus mode preparation")
            .admit_with(|request, _timeout| request);
            assert!(matches!(
                request,
                crate::runtime::engine::RuntimeRequest::Command {
                    applied_state: None,
                    ..
                }
            ));
        }
    }

    #[test]
    fn typed_request_inventory_covers_exactly_the_plain_ledger_rows() {
        let plain_rows: HashSet<_> = crate::command::semantics::BuiltinCommand::ALL
            .iter()
            .copied()
            .filter(|row| row.is_plain())
            .collect();
        let inventory_rows: HashSet<_> = BUILTIN_TYPED_REQUEST_INVENTORY
            .iter()
            .map(|entry| {
                assert!(entry.type_is_plain(), "{} is not Plain", entry.type_name);
                assert!(
                    !entry.branch.is_empty(),
                    "{} has no command branch",
                    entry.type_name
                );
                assert!(
                    entry.row.is_plain(),
                    "operation row in inventory: {:?}",
                    entry.row
                );
                entry.row
            })
            .collect();
        assert_eq!(inventory_rows, plain_rows);
        // Set equality alone cannot see a row listed twice, so pin the entry
        // count against the ledger the set came from rather than against the
        // inventory's own length.
        assert_eq!(
            BUILTIN_TYPED_REQUEST_INVENTORY.len(),
            plain_rows.len(),
            "the typed request inventory repeats a Plain ledger row"
        );
    }

    #[test]
    fn physical_requests_have_closed_classes_and_exact_axes() {
        assert_targeted::<IrisReset>();
        assert_targeted::<IrisUp>();
        assert_targeted::<IrisDown>();
        assert_targeted::<IrisDirect>();
        assert_targeted::<NdFilterDirect>();
        assert_targeted::<NdFilterStepUp>();
        assert_targeted::<NdFilterStepDown>();
        assert_applied_only::<PushAfPress>();
        assert_applied_only::<PushAfRelease>();

        assert_eq!(IrisReset.affected_axes(), AffectedAxes::IRIS);
        assert_eq!(IrisUp.affected_axes(), AffectedAxes::IRIS);
        assert_eq!(IrisDown.affected_axes(), AffectedAxes::IRIS);
        assert_eq!(
            IrisDirect::new(IrisLevel::new(4).unwrap()).affected_axes(),
            AffectedAxes::IRIS
        );
        assert_eq!(
            NdFilterDirect::new(NdFilterValue::new(4).unwrap()).affected_axes(),
            AffectedAxes::ND_FILTER
        );
        assert_eq!(NdFilterStepUp.affected_axes(), AffectedAxes::ND_FILTER);
        assert_eq!(NdFilterStepDown.affected_axes(), AffectedAxes::ND_FILTER);
        assert_eq!(PushAfPress.affected_axes(), AffectedAxes::FOCUS);
        assert_eq!(PushAfRelease.affected_axes(), AffectedAxes::FOCUS);

        assert_policy::<IrisReset>(
            6,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<IrisUp>(
            6,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<IrisDown>(
            6,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<IrisDirect>(
            9,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<NdFilterDirect>(
            9,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<NdFilterStepUp>(
            7,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<NdFilterStepDown>(
            7,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<PushAfPress>(
            8,
            TimeoutClass::Quick,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<PushAfRelease>(
            8,
            TimeoutClass::Quick,
            RetryClass::Movement,
            ControlClass::User,
        );
    }

    #[test]
    fn physical_requests_match_command_wire_encoders() {
        assert_eq!(wire(&IrisReset), command_wire(&Iris::Reset));
        assert_eq!(wire(&IrisUp), command_wire(&Iris::Up));
        assert_eq!(wire(&IrisDown), command_wire(&Iris::Down));

        let iris = IrisLevel::new(4).unwrap();
        assert_eq!(
            wire(&IrisDirect::new(iris)),
            command_wire(&Iris::SetAperture(iris))
        );

        let nd = NdFilterValue::new(4).unwrap();
        assert_eq!(wire(&NdFilterDirect::new(nd)), command_wire(&nd));
        assert_eq!(
            wire(&NdFilterStepUp),
            command_wire(&NdFilterStepCommand::new(NdFilterStep::Up))
        );
        assert_eq!(
            wire(&NdFilterStepDown),
            command_wire(&NdFilterStepCommand::new(NdFilterStep::Down))
        );
        assert_eq!(wire(&PushAfPress), command_wire(&PushAF::Press));
        assert_eq!(wire(&PushAfRelease), command_wire(&PushAF::Release));
    }

    #[test]
    fn profile_validation_rejects_unsupported_physical_requests_before_encoding() {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
        let nd = NdFilterDirect::new(NdFilterValue::new(4).unwrap());

        reset_request_write_count();
        for result in [
            prepare_builtin_operation::<completion::Targeted, _>(
                &IrisReset,
                CameraId::CAMERA_1,
                &profile,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &IrisUp,
                CameraId::CAMERA_1,
                &profile,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &IrisDown,
                CameraId::CAMERA_1,
                &profile,
                OperationalTuning::new(),
            ),
        ] {
            assert!(result.is_err());
        }
        let iris_direct = IrisDirect::new(IrisLevel::new(4).unwrap());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &iris_direct,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .is_err());
        for result in [
            prepare_builtin_operation::<completion::Targeted, _>(
                &nd,
                CameraId::CAMERA_1,
                &profile,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &NdFilterStepUp,
                CameraId::CAMERA_1,
                &profile,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &NdFilterStepDown,
                CameraId::CAMERA_1,
                &profile,
                OperationalTuning::new(),
            ),
        ] {
            assert!(result.is_err());
        }
        for result in [
            prepare_builtin_operation::<completion::AppliedOnly, _>(
                &PushAfPress,
                CameraId::CAMERA_1,
                &profile,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::AppliedOnly, _>(
                &PushAfRelease,
                CameraId::CAMERA_1,
                &profile,
                OperationalTuning::new(),
            ),
        ] {
            assert!(result.is_err());
        }
        assert_eq!(request_write_count(), 0);
    }

    #[test]
    fn supported_profiles_admit_iris_nd_and_push_af_requests() {
        let ptz = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZ profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");

        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &IrisReset,
            CameraId::CAMERA_1,
            &ptz,
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &IrisDirect::new(IrisLevel::new(4).unwrap()),
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &NdFilterDirect::new(NdFilterValue::new(4).unwrap()),
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &NdFilterStepUp,
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::AppliedOnly, _>(
            &PushAfPress,
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .is_ok());
    }
}
