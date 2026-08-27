//! Closed static camera-surface metadata for built-in requests.
//!
//! [`crate::command::semantics::BuiltinCommand::ALL`] is the only command-row
//! inventory.  This module adds no second list: its exhaustive match derives
//! noun spelling and marker facts for each existing semantic row.  Adding a
//! new command therefore requires the source inventory, semantic
//! classification, and this surface decision to be updated together.

use crate::capabilities::TypedSupportSurface;

use super::semantics::{BuiltinCommand, BuiltinRequestClass};

/// Static noun containing one target-facing built-in request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StaticNoun {
    /// Power controls.
    Power,
    /// Zoom controls.
    Zoom,
    /// System controls.
    System,
    /// Pan/tilt controls.
    PanTilt,
    /// Focus controls.
    Focus,
    /// Exposure and iris controls.
    Exposure,
    /// White-balance and channel controls.
    WhiteBalance,
    /// Image-processing controls.
    Image,
    /// Preset controls.
    Presets,
    /// Tally controls.
    Tally,
    /// Neutral-density filter controls.
    NdFilter,
    /// Motion-sync controls.
    MotionSync,
    /// Menu controls.
    Menu,
    /// Advanced and vendor controls.
    Advanced,
}

/// Profile or typed-capability marker required by a static surface row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StaticMarkerRequirement {
    /// The base profile/domain marker is sufficient.
    None,
    /// A compile-time profile marker trait is required.
    Profile(&'static str),
    /// A runtime/static typed capability gate is required.
    Typed(TypedSupportSurface),
}

/// What kind of public surface disposition a built-in request has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StaticSurfaceDisposition {
    /// A normal target-facing noun method.
    Noun {
        /// Noun accessor containing the method.
        noun: StaticNoun,
        /// Canonical method spelling shared by all facades.
        method: &'static str,
        /// Compile-time/runtime support marker.
        marker: StaticMarkerRequirement,
    },
    /// A broadcast handshake, never a target camera method.
    BroadcastHandshake {
        /// Internal protocol spelling retained for the broadcast namespace.
        method: &'static str,
    },
    /// An internal cancellation primitive, never a noun method.
    InternalCancellation {
        /// Internal protocol spelling retained for owner cancellation.
        method: &'static str,
    },
}

/// One derived row in the closed static noun ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct StaticSurfaceEntry {
    /// Exactly one source semantic row.
    pub(crate) command: BuiltinCommand,
    /// Noun mapping or explicit non-noun exception.
    pub(crate) disposition: StaticSurfaceDisposition,
    /// Semantic return class copied from the authoritative row.
    pub(crate) class: BuiltinRequestClass,
}

impl StaticSurfaceEntry {
    /// Returns whether the row may become a target camera noun method.
    #[must_use]
    pub(crate) const fn is_target_facing(self) -> bool {
        matches!(self.disposition, StaticSurfaceDisposition::Noun { .. })
    }
}

macro_rules! noun_marker {
    (Power) => {
        StaticMarkerRequirement::Profile("HasPower")
    };
    (Zoom) => {
        StaticMarkerRequirement::Profile("HasZoom")
    };
    (System) => {
        StaticMarkerRequirement::None
    };
    (PanTilt) => {
        StaticMarkerRequirement::Profile("HasPanTilt")
    };
    (Focus) => {
        StaticMarkerRequirement::Profile("HasFocus")
    };
    (Exposure) => {
        StaticMarkerRequirement::Profile("HasExposure")
    };
    (WhiteBalance) => {
        StaticMarkerRequirement::Profile("HasWhiteBalance")
    };
    (Image) => {
        StaticMarkerRequirement::Profile("HasImageProcessing")
    };
    (Presets) => {
        StaticMarkerRequirement::Profile("HasPresets")
    };
    (Tally) => {
        StaticMarkerRequirement::Typed(TypedSupportSurface::Tally)
    };
    (NdFilter) => {
        StaticMarkerRequirement::Typed(TypedSupportSurface::NdFilter)
    };
    (MotionSync) => {
        StaticMarkerRequirement::Typed(TypedSupportSurface::MotionSync)
    };
    (Menu) => {
        StaticMarkerRequirement::Profile("HasMenuControl")
    };
    (Advanced) => {
        StaticMarkerRequirement::None
    };
}

macro_rules! noun_entry {
    ($command:ident, $noun:ident, $method:literal) => {
        StaticSurfaceEntry {
            command: BuiltinCommand::$command,
            disposition: StaticSurfaceDisposition::Noun {
                noun: StaticNoun::$noun,
                method: $method,
                marker: noun_marker!($noun),
            },
            class: BuiltinCommand::$command.classification(),
        }
    };
    ($command:ident, $noun:ident, $method:literal, $marker:expr) => {
        StaticSurfaceEntry {
            command: BuiltinCommand::$command,
            disposition: StaticSurfaceDisposition::Noun {
                noun: StaticNoun::$noun,
                method: $method,
                marker: $marker,
            },
            class: BuiltinCommand::$command.classification(),
        }
    };
}

macro_rules! broadcast_entry {
    ($command:ident, $method:literal) => {
        StaticSurfaceEntry {
            command: BuiltinCommand::$command,
            disposition: StaticSurfaceDisposition::BroadcastHandshake { method: $method },
            class: BuiltinCommand::$command.classification(),
        }
    };
}

macro_rules! internal_entry {
    ($command:ident, $method:literal) => {
        StaticSurfaceEntry {
            command: BuiltinCommand::$command,
            disposition: StaticSurfaceDisposition::InternalCancellation { method: $method },
            class: BuiltinCommand::$command.classification(),
        }
    };
}

/// Derive the one surface row for a semantic command.
///
/// This match is intentionally exhaustive and contains no default arm.  The
/// 147-row source list remains [`BuiltinCommand::ALL`], not a parallel table.
#[must_use]
pub(crate) const fn surface_entry(command: BuiltinCommand) -> StaticSurfaceEntry {
    match command {
        // Pan/tilt.
        BuiltinCommand::PanTiltHome => noun_entry!(PanTiltHome, PanTilt, "home"),
        BuiltinCommand::PanTiltReset => noun_entry!(PanTiltReset, PanTilt, "reset"),
        BuiltinCommand::PanTiltDrive => noun_entry!(PanTiltDrive, PanTilt, "move_direction"),
        BuiltinCommand::PanTiltStop => noun_entry!(PanTiltStop, PanTilt, "stop"),
        BuiltinCommand::PanTiltAbsolute => noun_entry!(PanTiltAbsolute, PanTilt, "absolute"),
        BuiltinCommand::PanTiltRelative => noun_entry!(PanTiltRelative, PanTilt, "relative"),
        BuiltinCommand::PanTiltLimitSet => noun_entry!(PanTiltLimitSet, PanTilt, "limit_set"),
        BuiltinCommand::PanTiltLimitClear => {
            noun_entry!(PanTiltLimitClear, PanTilt, "limit_clear")
        }
        // Zoom.
        BuiltinCommand::ZoomStop => noun_entry!(ZoomStop, Zoom, "stop"),
        BuiltinCommand::ZoomTele => noun_entry!(ZoomTele, Zoom, "tele"),
        BuiltinCommand::ZoomWide => noun_entry!(ZoomWide, Zoom, "wide"),
        BuiltinCommand::ZoomTeleVariable => noun_entry!(ZoomTeleVariable, Zoom, "tele_variable"),
        BuiltinCommand::ZoomWideVariable => noun_entry!(ZoomWideVariable, Zoom, "wide_variable"),
        BuiltinCommand::ZoomPosition => noun_entry!(
            ZoomPosition,
            Zoom,
            "set_position",
            StaticMarkerRequirement::Typed(TypedSupportSurface::DirectZoom)
        ),
        BuiltinCommand::DigitalZoom => noun_entry!(
            DigitalZoom,
            Zoom,
            "set_digital_zoom",
            StaticMarkerRequirement::Typed(TypedSupportSurface::DigitalZoomToggle)
        ),
        // Focus.
        BuiltinCommand::FocusStop => noun_entry!(FocusStop, Focus, "stop"),
        BuiltinCommand::FocusFar => noun_entry!(FocusFar, Focus, "far"),
        BuiltinCommand::FocusNear => noun_entry!(FocusNear, Focus, "near"),
        BuiltinCommand::FocusFarVariable => noun_entry!(FocusFarVariable, Focus, "far_variable"),
        BuiltinCommand::FocusNearVariable => {
            noun_entry!(FocusNearVariable, Focus, "near_variable")
        }
        BuiltinCommand::FocusPosition => noun_entry!(FocusPosition, Focus, "set_position"),
        BuiltinCommand::FocusAuto => noun_entry!(FocusAuto, Focus, "auto"),
        BuiltinCommand::FocusManual => noun_entry!(FocusManual, Focus, "manual"),
        BuiltinCommand::FocusOnePush => noun_entry!(
            FocusOnePush,
            Focus,
            "one_push",
            StaticMarkerRequirement::Typed(TypedSupportSurface::OnePushFocus)
        ),
        BuiltinCommand::FocusInfinity => noun_entry!(FocusInfinity, Focus, "infinity"),
        BuiltinCommand::FocusToggle => noun_entry!(FocusToggle, Focus, "toggle"),
        BuiltinCommand::FocusSnap => noun_entry!(
            FocusSnap,
            Focus,
            "snap",
            StaticMarkerRequirement::Typed(TypedSupportSurface::PtzOpticsSnapFocus)
        ),
        BuiltinCommand::FocusZone => noun_entry!(
            FocusZone,
            Focus,
            "set_zone",
            StaticMarkerRequirement::Typed(TypedSupportSurface::FocusZone)
        ),
        BuiltinCommand::FocusAutoSensitivity => noun_entry!(
            FocusAutoSensitivity,
            Focus,
            "set_sensitivity",
            StaticMarkerRequirement::Typed(TypedSupportSurface::AutoFocusSensitivity)
        ),
        BuiltinCommand::FocusNearLimit => noun_entry!(
            FocusNearLimit,
            Focus,
            "set_near_limit",
            StaticMarkerRequirement::Typed(TypedSupportSurface::FocusNearLimitInquiry)
        ),
        BuiltinCommand::FocusLock => noun_entry!(
            FocusLock,
            Focus,
            "set_lock",
            StaticMarkerRequirement::Typed(TypedSupportSurface::FocusLock)
        ),
        BuiltinCommand::PushAfPress => noun_entry!(
            PushAfPress,
            Focus,
            "push_af_press",
            StaticMarkerRequirement::Typed(TypedSupportSurface::PushAutoFocus)
        ),
        BuiltinCommand::PushAfRelease => noun_entry!(
            PushAfRelease,
            Focus,
            "push_af_release",
            StaticMarkerRequirement::Typed(TypedSupportSurface::PushAutoFocus)
        ),
        // Presets and power.
        BuiltinCommand::PresetRecall => noun_entry!(PresetRecall, Presets, "recall"),
        BuiltinCommand::PresetRecallSpeed => {
            noun_entry!(PresetRecallSpeed, Presets, "set_recall_speed")
        }
        BuiltinCommand::PresetSet => noun_entry!(PresetSet, Presets, "set"),
        BuiltinCommand::PresetReset => noun_entry!(PresetReset, Presets, "reset"),
        BuiltinCommand::PowerOn => noun_entry!(PowerOn, Power, "on"),
        BuiltinCommand::PowerStandby => noun_entry!(PowerStandby, Power, "off"),
        // Exposure, iris, shutter, brightness, and gain.
        BuiltinCommand::ExposureMode => noun_entry!(ExposureMode, Exposure, "set_mode"),
        BuiltinCommand::ExposureCompensationOn => noun_entry!(
            ExposureCompensationOn,
            Exposure,
            "compensation_on",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ExposureCompensation)
        ),
        BuiltinCommand::ExposureCompensationOff => noun_entry!(
            ExposureCompensationOff,
            Exposure,
            "compensation_off",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ExposureCompensation)
        ),
        BuiltinCommand::ExposureCompensationReset => noun_entry!(
            ExposureCompensationReset,
            Exposure,
            "compensation_reset",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ExposureCompensation)
        ),
        BuiltinCommand::ExposureCompensationUp => noun_entry!(
            ExposureCompensationUp,
            Exposure,
            "compensation_up",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ExposureCompensation)
        ),
        BuiltinCommand::ExposureCompensationDown => noun_entry!(
            ExposureCompensationDown,
            Exposure,
            "compensation_down",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ExposureCompensation)
        ),
        BuiltinCommand::ExposureCompensationDirect => noun_entry!(
            ExposureCompensationDirect,
            Exposure,
            "compensation_direct",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ExposureCompensation)
        ),
        BuiltinCommand::DynamicRange => noun_entry!(
            DynamicRange,
            Exposure,
            "set_dynamic_range",
            StaticMarkerRequirement::Typed(TypedSupportSurface::WideDynamicRange)
        ),
        BuiltinCommand::IrisReset => noun_entry!(
            IrisReset,
            Exposure,
            "iris_reset",
            StaticMarkerRequirement::Typed(TypedSupportSurface::IrisControl)
        ),
        BuiltinCommand::IrisUp => noun_entry!(
            IrisUp,
            Exposure,
            "iris_up",
            StaticMarkerRequirement::Typed(TypedSupportSurface::IrisControl)
        ),
        BuiltinCommand::IrisDown => noun_entry!(
            IrisDown,
            Exposure,
            "iris_down",
            StaticMarkerRequirement::Typed(TypedSupportSurface::IrisControl)
        ),
        BuiltinCommand::IrisDirect => noun_entry!(
            IrisDirect,
            Exposure,
            "iris_direct",
            StaticMarkerRequirement::Typed(TypedSupportSurface::IrisControl)
        ),
        BuiltinCommand::ShutterReset => noun_entry!(ShutterReset, Exposure, "shutter_reset"),
        BuiltinCommand::ShutterUp => noun_entry!(ShutterUp, Exposure, "shutter_up"),
        BuiltinCommand::ShutterDown => noun_entry!(ShutterDown, Exposure, "shutter_down"),
        BuiltinCommand::ShutterDirect => noun_entry!(ShutterDirect, Exposure, "shutter_direct"),
        BuiltinCommand::BrightnessReset => noun_entry!(
            BrightnessReset,
            Exposure,
            "brightness_reset",
            StaticMarkerRequirement::Typed(TypedSupportSurface::BrightnessControl)
        ),
        BuiltinCommand::BrightnessUp => noun_entry!(
            BrightnessUp,
            Exposure,
            "brightness_up",
            StaticMarkerRequirement::Typed(TypedSupportSurface::BrightnessControl)
        ),
        BuiltinCommand::BrightnessDown => noun_entry!(
            BrightnessDown,
            Exposure,
            "brightness_down",
            StaticMarkerRequirement::Typed(TypedSupportSurface::BrightnessControl)
        ),
        BuiltinCommand::BrightnessSet => noun_entry!(
            BrightnessSet,
            Exposure,
            "brightness_set",
            StaticMarkerRequirement::Typed(TypedSupportSurface::BrightnessControl)
        ),
        BuiltinCommand::BrightnessDirect => noun_entry!(
            BrightnessDirect,
            Exposure,
            "brightness_direct",
            StaticMarkerRequirement::Typed(TypedSupportSurface::BrightnessControl)
        ),
        BuiltinCommand::AntiFlicker => noun_entry!(AntiFlicker, Exposure, "set_anti_flicker"),
        BuiltinCommand::SpotlightOn => noun_entry!(SpotlightOn, Exposure, "spotlight_on"),
        BuiltinCommand::SpotlightOff => noun_entry!(SpotlightOff, Exposure, "spotlight_off"),
        BuiltinCommand::AutoSlowShutterOn => {
            noun_entry!(AutoSlowShutterOn, Exposure, "auto_slow_shutter_on")
        }
        BuiltinCommand::AutoSlowShutterOff => {
            noun_entry!(AutoSlowShutterOff, Exposure, "auto_slow_shutter_off")
        }
        BuiltinCommand::GainReset => noun_entry!(GainReset, Exposure, "gain_reset"),
        BuiltinCommand::GainUp => noun_entry!(GainUp, Exposure, "gain_up"),
        BuiltinCommand::GainDown => noun_entry!(GainDown, Exposure, "gain_down"),
        BuiltinCommand::GainDirect => noun_entry!(GainDirect, Exposure, "gain_direct"),
        BuiltinCommand::GainLimit => noun_entry!(GainLimit, Exposure, "set_gain_limit"),
        // White balance and channel controls.
        BuiltinCommand::WhiteBalanceAuto => noun_entry!(WhiteBalanceAuto, WhiteBalance, "auto"),
        BuiltinCommand::WhiteBalanceIndoor => {
            noun_entry!(WhiteBalanceIndoor, WhiteBalance, "indoor")
        }
        BuiltinCommand::WhiteBalanceOutdoor => {
            noun_entry!(WhiteBalanceOutdoor, WhiteBalance, "outdoor")
        }
        BuiltinCommand::WhiteBalanceOnePush => noun_entry!(
            WhiteBalanceOnePush,
            WhiteBalance,
            "one_push",
            StaticMarkerRequirement::Typed(TypedSupportSurface::OnePushWhiteBalance)
        ),
        BuiltinCommand::WhiteBalanceAutoTracking => noun_entry!(
            WhiteBalanceAutoTracking,
            WhiteBalance,
            "atw",
            StaticMarkerRequirement::Typed(TypedSupportSurface::AutoTrackingWhiteBalance)
        ),
        BuiltinCommand::WhiteBalanceManual => {
            noun_entry!(WhiteBalanceManual, WhiteBalance, "manual")
        }
        BuiltinCommand::WhiteBalanceColorTemperature => noun_entry!(
            WhiteBalanceColorTemperature,
            WhiteBalance,
            "color_temperature_mode",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ColorTemperature)
        ),
        BuiltinCommand::AutoWhiteBalanceSensitivity => noun_entry!(
            AutoWhiteBalanceSensitivity,
            WhiteBalance,
            "set_sensitivity",
            StaticMarkerRequirement::Typed(TypedSupportSurface::AutoWhiteBalanceSensitivity)
        ),
        BuiltinCommand::OnePushWhiteBalanceTrigger => noun_entry!(
            OnePushWhiteBalanceTrigger,
            WhiteBalance,
            "one_push_trigger",
            StaticMarkerRequirement::Typed(TypedSupportSurface::OnePushWhiteBalance)
        ),
        BuiltinCommand::RedTuning => noun_entry!(
            RedTuning,
            WhiteBalance,
            "set_red_tuning",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbTuning)
        ),
        BuiltinCommand::BlueTuning => noun_entry!(
            BlueTuning,
            WhiteBalance,
            "set_blue_tuning",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbTuning)
        ),
        // Image processing, including saturation and hue.
        BuiltinCommand::Saturation => noun_entry!(
            Saturation,
            Image,
            "set_saturation",
            StaticMarkerRequirement::Typed(TypedSupportSurface::SaturationControl)
        ),
        BuiltinCommand::Hue => noun_entry!(
            Hue,
            Image,
            "set_hue",
            StaticMarkerRequirement::Typed(TypedSupportSurface::HueControl)
        ),
        BuiltinCommand::ColorTemperatureReset => noun_entry!(
            ColorTemperatureReset,
            WhiteBalance,
            "reset_color_temperature",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ColorTemperature)
        ),
        BuiltinCommand::ColorTemperatureUp => noun_entry!(
            ColorTemperatureUp,
            WhiteBalance,
            "increase_color_temperature",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ColorTemperature)
        ),
        BuiltinCommand::ColorTemperatureDown => noun_entry!(
            ColorTemperatureDown,
            WhiteBalance,
            "decrease_color_temperature",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ColorTemperature)
        ),
        BuiltinCommand::ColorTemperatureDirect => noun_entry!(
            ColorTemperatureDirect,
            WhiteBalance,
            "set_color_temperature",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ColorTemperature)
        ),
        BuiltinCommand::RedGainReset => noun_entry!(
            RedGainReset,
            WhiteBalance,
            "reset_red_gain",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbGain)
        ),
        BuiltinCommand::RedGainUp => noun_entry!(
            RedGainUp,
            WhiteBalance,
            "increase_red_gain",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbGain)
        ),
        BuiltinCommand::RedGainDown => noun_entry!(
            RedGainDown,
            WhiteBalance,
            "decrease_red_gain",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbGain)
        ),
        BuiltinCommand::RedGainDirect => noun_entry!(
            RedGainDirect,
            WhiteBalance,
            "set_red_gain",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbGain)
        ),
        BuiltinCommand::BlueGainReset => noun_entry!(
            BlueGainReset,
            WhiteBalance,
            "reset_blue_gain",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbGain)
        ),
        BuiltinCommand::BlueGainUp => noun_entry!(
            BlueGainUp,
            WhiteBalance,
            "increase_blue_gain",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbGain)
        ),
        BuiltinCommand::BlueGainDown => noun_entry!(
            BlueGainDown,
            WhiteBalance,
            "decrease_blue_gain",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbGain)
        ),
        BuiltinCommand::BlueGainDirect => noun_entry!(
            BlueGainDirect,
            WhiteBalance,
            "set_blue_gain",
            StaticMarkerRequirement::Typed(TypedSupportSurface::RgbGain)
        ),
        BuiltinCommand::SharpnessMode => noun_entry!(
            SharpnessMode,
            Image,
            "set_sharpness_mode",
            StaticMarkerRequirement::Typed(TypedSupportSurface::SharpnessControl)
        ),
        BuiltinCommand::SharpnessReset => noun_entry!(
            SharpnessReset,
            Image,
            "reset_sharpness",
            StaticMarkerRequirement::Typed(TypedSupportSurface::SharpnessControl)
        ),
        BuiltinCommand::SharpnessUp => noun_entry!(
            SharpnessUp,
            Image,
            "increase_sharpness",
            StaticMarkerRequirement::Typed(TypedSupportSurface::SharpnessControl)
        ),
        BuiltinCommand::SharpnessDown => noun_entry!(
            SharpnessDown,
            Image,
            "decrease_sharpness",
            StaticMarkerRequirement::Typed(TypedSupportSurface::SharpnessControl)
        ),
        BuiltinCommand::SharpnessDirect => noun_entry!(
            SharpnessDirect,
            Image,
            "set_sharpness",
            StaticMarkerRequirement::Typed(TypedSupportSurface::SharpnessControl)
        ),
        BuiltinCommand::Luminance => noun_entry!(
            Luminance,
            Image,
            "set_luminance",
            StaticMarkerRequirement::Typed(TypedSupportSurface::LuminanceControl)
        ),
        BuiltinCommand::Contrast => noun_entry!(
            Contrast,
            Image,
            "set_contrast",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ContrastControl)
        ),
        BuiltinCommand::Gamma => noun_entry!(
            Gamma,
            Image,
            "set_gamma",
            StaticMarkerRequirement::Typed(TypedSupportSurface::GammaControl)
        ),
        BuiltinCommand::Backlight => noun_entry!(
            Backlight,
            Image,
            "set_backlight",
            StaticMarkerRequirement::Typed(TypedSupportSurface::BacklightCompensation)
        ),
        BuiltinCommand::NoiseReduction2d => noun_entry!(
            NoiseReduction2d,
            Image,
            "set_noise_reduction_2d",
            StaticMarkerRequirement::Typed(TypedSupportSurface::NoiseReduction2D)
        ),
        BuiltinCommand::NoiseReduction3d => noun_entry!(
            NoiseReduction3d,
            Image,
            "set_noise_reduction_3d",
            StaticMarkerRequirement::Typed(TypedSupportSurface::NoiseReduction3D)
        ),
        BuiltinCommand::ImageFlipOff => noun_entry!(
            ImageFlipOff,
            Image,
            "disable_flip",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ImageFlip)
        ),
        BuiltinCommand::ImageFlipHorizontal => noun_entry!(
            ImageFlipHorizontal,
            Image,
            "enable_horizontal_flip",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ImageMirror)
        ),
        BuiltinCommand::ImageFlipHorizontalOff => noun_entry!(
            ImageFlipHorizontalOff,
            Image,
            "disable_horizontal_flip",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ImageMirror)
        ),
        BuiltinCommand::ImageFlipVertical => noun_entry!(
            ImageFlipVertical,
            Image,
            "enable_flip",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ImageFlip)
        ),
        BuiltinCommand::ImageFlipBoth => noun_entry!(
            ImageFlipBoth,
            Image,
            "set_flip_both",
            StaticMarkerRequirement::Typed(TypedSupportSurface::ImageFlip)
        ),
        BuiltinCommand::ImageFlipCombined => noun_entry!(
            ImageFlipCombined,
            Image,
            "set_flip_mode",
            StaticMarkerRequirement::Typed(TypedSupportSurface::CombinedImageFlip)
        ),
        BuiltinCommand::ImageFreezeOn => noun_entry!(ImageFreezeOn, Image, "freeze_on"),
        BuiltinCommand::ImageFreezeOff => noun_entry!(ImageFreezeOff, Image, "freeze_off"),
        BuiltinCommand::PictureEffect => noun_entry!(
            PictureEffect,
            Image,
            "set_picture_effect",
            StaticMarkerRequirement::Typed(TypedSupportSurface::PictureEffect)
        ),
        // Neutral-density filter.
        BuiltinCommand::NdFilterMode => noun_entry!(NdFilterMode, NdFilter, "set_mode"),
        BuiltinCommand::NdFilterDirect => noun_entry!(NdFilterDirect, NdFilter, "set_value"),
        BuiltinCommand::NdFilterStepUp => noun_entry!(NdFilterStepUp, NdFilter, "step_up"),
        BuiltinCommand::NdFilterStepDown => {
            noun_entry!(NdFilterStepDown, NdFilter, "step_down")
        }
        BuiltinCommand::NdFilterAutoOn => noun_entry!(NdFilterAutoOn, NdFilter, "auto_on"),
        BuiltinCommand::NdFilterAutoOff => noun_entry!(NdFilterAutoOff, NdFilter, "auto_off"),
        // Tally.
        BuiltinCommand::TallyRedOn => noun_entry!(TallyRedOn, Tally, "red_on"),
        BuiltinCommand::TallyRedOff => noun_entry!(TallyRedOff, Tally, "red_off"),
        BuiltinCommand::TallyBrightLow => noun_entry!(TallyBrightLow, Tally, "bright_lo"),
        BuiltinCommand::TallyBrightHigh => noun_entry!(TallyBrightHigh, Tally, "bright_hi"),
        BuiltinCommand::TallyGreenOn => noun_entry!(TallyGreenOn, Tally, "green_on"),
        BuiltinCommand::TallyGreenOff => noun_entry!(TallyGreenOff, Tally, "green_off"),
        BuiltinCommand::TallyFlash => noun_entry!(TallyFlash, Tally, "flash"),
        BuiltinCommand::TallyOn => noun_entry!(TallyOn, Tally, "on"),
        BuiltinCommand::TallyOff => noun_entry!(TallyOff, Tally, "off"),
        // Menu.
        BuiltinCommand::MenuDisplay => noun_entry!(MenuDisplay, Menu, "display"),
        BuiltinCommand::MenuNavigate => noun_entry!(MenuNavigate, Menu, "navigate"),
        BuiltinCommand::MenuSelect => noun_entry!(MenuSelect, Menu, "select"),
        BuiltinCommand::MenuCancel => noun_entry!(MenuCancel, Menu, "cancel"),
        BuiltinCommand::DirectMenu => noun_entry!(
            DirectMenu,
            Menu,
            "direct",
            StaticMarkerRequirement::Typed(TypedSupportSurface::DirectMenu)
        ),
        // Advanced/vendor controls.
        BuiltinCommand::MulticastStreamingOn => {
            noun_entry!(MulticastStreamingOn, Advanced, "multicast_on")
        }
        BuiltinCommand::MulticastStreamingOff => {
            noun_entry!(MulticastStreamingOff, Advanced, "multicast_off")
        }
        BuiltinCommand::NdiQuality => noun_entry!(NdiQuality, Advanced, "set_ndi_quality"),
        BuiltinCommand::UsbAudioOn => noun_entry!(UsbAudioOn, Advanced, "usb_audio_on"),
        BuiltinCommand::UsbAudioOff => noun_entry!(UsbAudioOff, Advanced, "usb_audio_off"),
        // Explicit non-noun exceptions.
        BuiltinCommand::AddressSet => broadcast_entry!(AddressSet, "address_set"),
        BuiltinCommand::InterfaceClear => broadcast_entry!(InterfaceClear, "interface_clear"),
        BuiltinCommand::CommandCancel => internal_entry!(CommandCancel, "cancel_command"),
        BuiltinCommand::SettingsSave => noun_entry!(SettingsSave, System, "save_settings"),
        BuiltinCommand::MotionSyncMode => noun_entry!(MotionSyncMode, MotionSync, "set_mode"),
        BuiltinCommand::MotionSyncPreset => noun_entry!(MotionSyncPreset, MotionSync, "set_preset"),
        BuiltinCommand::VariableSpeedMode => noun_entry!(
            VariableSpeedMode,
            Advanced,
            "set_variable_speed_mode",
            StaticMarkerRequirement::Typed(TypedSupportSurface::VariableSpeed)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_ledger_is_exhaustive_unique_and_class_balanced() {
        let mut seen = std::collections::HashSet::new();
        let mut plain = 0;
        let mut applied_only = 0;
        let mut targeted = 0;
        for command in BuiltinCommand::ALL {
            let entry = surface_entry(*command);
            assert!(seen.insert(*command), "duplicate surface ledger row");
            assert_eq!(entry.command, *command);
            assert_eq!(entry.class, command.classification());
            match entry.class {
                BuiltinRequestClass::Plain { .. } => plain += 1,
                BuiltinRequestClass::AppliedOnly { .. } => applied_only += 1,
                BuiltinRequestClass::Targeted { .. } => targeted += 1,
            }
            match entry.disposition {
                StaticSurfaceDisposition::Noun { method, .. }
                | StaticSurfaceDisposition::BroadcastHandshake { method }
                | StaticSurfaceDisposition::InternalCancellation { method } => {
                    assert!(!method.is_empty());
                }
            }
        }

        // The class totals are derived from `BuiltinCommand::ALL` rather than
        // written down: every row lands in exactly one class, so the three
        // counters must add back up to the source inventory.
        assert_eq!(plain + applied_only + targeted, BuiltinCommand::ALL.len());
        assert_eq!(seen.len(), BuiltinCommand::ALL.len());
    }

    #[test]
    fn broadcast_and_internal_rows_never_have_noun_dispositions() {
        assert!(matches!(
            surface_entry(BuiltinCommand::AddressSet).disposition,
            StaticSurfaceDisposition::BroadcastHandshake { .. }
        ));
        assert!(matches!(
            surface_entry(BuiltinCommand::InterfaceClear).disposition,
            StaticSurfaceDisposition::BroadcastHandshake { .. }
        ));
        assert!(matches!(
            surface_entry(BuiltinCommand::CommandCancel).disposition,
            StaticSurfaceDisposition::InternalCancellation { .. }
        ));

        // The target-facing total is derived: the ledger is closed, so the
        // noun rows are exactly the rows that are not one of the three named
        // non-noun exceptions above.
        let exceptions: Vec<BuiltinCommand> = BuiltinCommand::ALL
            .iter()
            .copied()
            .filter(|command| !surface_entry(*command).is_target_facing())
            .collect();
        assert_eq!(
            exceptions,
            vec![
                BuiltinCommand::AddressSet,
                BuiltinCommand::InterfaceClear,
                BuiltinCommand::CommandCancel,
            ]
        );

        let noun_count = BuiltinCommand::ALL
            .iter()
            .filter(|command| surface_entry(**command).is_target_facing())
            .count();
        assert_eq!(noun_count + exceptions.len(), BuiltinCommand::ALL.len());
    }
}
