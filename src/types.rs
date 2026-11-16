//! Checked values, settings and physical quantities used by camera controls.
//!
//! Constructors validate each value's domain. Request preparation applies the
//! selected camera profile's narrower limits and encoding.

pub use crate::camera::{IdleWait, MotionQuery, MovementTolerance, PanTiltPosition, PanTiltUnits};
pub use crate::command::exposure_wire::{AntiFlickerMode, ExposureMode};
pub use crate::command::focus_wire::{AutoFocusSensitivity, FocusMode, FocusZone};
pub use crate::command::image_wire::{
    ImageFlipMode, NoiseReduction2D, NoiseReduction2DMode, NoiseReduction3D, SharpnessMode,
};
pub use crate::command::inquiry_types::VersionInfo;
pub use crate::command::menu_wire::{DirectMenuTarget, MenuButton, MenuDirection};
pub use crate::command::nd_filter_wire::{NdFilterDensity, NdFilterMode};
pub use crate::command::pan_tilt_wire::{PanTiltDirection, PanTiltLimitCorner};
pub use crate::command::power_wire::PowerStatus;
pub use crate::command::preset_wire::PresetNumber;
pub use crate::command::resolution::{PictureEffectMode, UnknownPictureEffect};
pub use crate::command::sony_fr7_exposure::{AeLevel, IrisStep};
pub use crate::command::sony_fr7_image::{
    BlackLevel, BlackStep, KneeMode, KneeParameter, KneePermission, KneePoint, KneeSlope,
};
pub use crate::command::sony_fr7_presets::{PresetSeparateMode, PresetSpeed, PresetSpeedSelection};
pub use crate::command::sony_fr7_white_balance::{
    MemoryColorTemperatureStep, WhiteBalanceGain, WhiteBalanceOffset, WhiteBalanceStep,
};
pub use crate::command::sony_fr7_zoom::{ClearImageZoomPosition, HighResolutionZoomSpeed};
pub use crate::command::variable_speed::PanTiltSpeedMode;
pub use crate::command::white_balance_wire::{AutoWhiteBalanceSensitivity, WhiteBalanceMode};

pub use crate::inquiry_conversions::ZoomDomain;
pub use crate::units::{Degrees, ExposureTime, Kelvin, UnitInterval};

use crate::error::Error;

/// Gain code for direct gain control.
///
/// The union domain includes BRC-H900 codes through `0x19` (R11 p. 36).
/// Request preparation checks the selected profile's gain range.
#[crate::value(min = 0x00, max = 0x19, display = "Gain Level {:#04x}")]
pub struct GainLevel(u8);

/// Maximum automatic gain code, checked against the selected profile at preparation.
#[crate::value(min = 0x00, max = 0x0F, display = "Gain Limit {:#04x}")]
pub struct GainLimit(u8);

/// Nonzero 2D noise reduction level; disabling the filter uses its `Off` setting.
#[crate::value(min = 1, max = 5, display = "2D NR Level {}")]
pub struct NoiseReduction2DLevel(u8);

/// Nonzero 3D noise reduction level; disabling the filter uses its `Off` setting.
#[crate::value(min = 1, max = 8, display = "3D NR Level {}")]
pub struct NoiseReduction3DLevel(u8);

/// Iris code in the union of the built-in profiles' direct iris domains.
///
/// Request preparation checks the selected profile's iris range.
#[crate::value(min = 0x00, max = 0x1E, display = "Iris {:#04x}")]
pub struct IrisLevel(u8);

/// Shutter code carried by a two-nibble shutter field.
///
/// Every byte is a syntactically valid code. The selected profile's shutter
/// table determines the exposure time and whether the code is supported.
#[crate::value(min = 0, max = 255, display = "Shutter {:#04x}")]
pub struct ShutterSpeed(u8);

/// Bright position carried by a two-nibble direct brightness field.
///
/// Request preparation checks the selected profile's brightness range.
#[crate::value(min = 0, max = 255, display = "Brightness {:#04x}")]
pub struct BrightnessLevel(u8);

/// A four-nibble response is a brightness code only when its upper byte is zero.
impl TryFrom<u16> for BrightnessLevel {
    type Error = Error;

    fn try_from(position: u16) -> Result<Self, Self::Error> {
        let position = u8::try_from(position).map_err(|_| {
            Error::parameter_out_of_range(
                "brightness_level",
                i32::from(position),
                0,
                i32::from(u8::MAX),
            )
        })?;
        Self::new(position)
    }
}

/// Sharpness code for direct adjustment.
#[crate::value(min = 0x00, max = 0x0F, display = "Sharpness {}")]
pub struct SharpnessLevel(u8);

/// Luminance code for direct adjustment.
#[crate::value(min = 0x00, max = 0x0E, display = "Luminance {}")]
pub struct LuminanceLevel(u8);

/// Contrast code for direct adjustment.
#[crate::value(min = 0x00, max = 0x0E, display = "Contrast {}")]
pub struct ContrastLevel(u8);

/// Wide dynamic range adjustment code, validated against the selected profile.
#[crate::value(min = 0x00, max = 0x08, display = "Wide Dynamic Range {}")]
pub struct WideDynamicRangeLevel(u8);

/// Exposure compensation level from -7 through +7; zero is neutral.
///
/// The wire code is the level plus seven.
#[crate::value(
    min = -7,
    max = 7,
    display = "Exposure Compensation {}",
    wire = centered(7)
)]
pub struct ExposureCompensationLevel(i8);

/// Coarse movement speed resolved against the selected profile during preparation.
///
/// This value has no profile-independent conversion into an axis speed. Pass
/// it directly to a control accepting [`crate::types::Speed<T>`].
#[crate::value_enum]
#[derive(Default)]
pub enum SpeedLevel {
    /// Slowest coarse movement level.
    Slowest = 0,
    /// Slow movement.
    Slow = 1,
    /// Balanced speed and control.
    #[default]
    Medium = 2,
    /// Fast movement.
    Fast = 3,
    /// Fastest coarse movement level.
    Fastest = 4,
}

impl SpeedLevel {
    /// Baseline pan, tilt, zoom and focus values consumed only by profile lowering.
    ///
    /// The preparation path clamps these using the profile's actual speed
    /// grammar, including BRC-300's shared pan/tilt speed field.
    pub(crate) const fn baseline_speeds(self) -> (u8, u8, u8, u8) {
        match self {
            Self::Slowest => (1, 1, 0, 0),
            Self::Slow => (6, 5, 2, 2),
            Self::Medium => (12, 10, 4, 4),
            Self::Fast => (18, 15, 6, 6),
            Self::Fastest => (PanTiltSpeedMode::STANDARD_MAXIMUM, 20, 7, 7),
        }
    }
}

/// A coarse profile-resolved speed or an exact checked speed.
///
/// Exact values retain their units and are checked against the selected
/// profile during preparation. A coarse level is lowered only after the
/// target profile is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum Speed<T> {
    /// Coarse speed interpreted using the selected profile.
    Level(SpeedLevel),
    /// Exact speed, still subject to the selected profile's range.
    Exact(T),
}

impl<T> From<SpeedLevel> for Speed<T> {
    fn from(value: SpeedLevel) -> Self {
        Self::Level(value)
    }
}

impl From<PanTiltSpeed> for Speed<PanTiltSpeed> {
    fn from(value: PanTiltSpeed) -> Self {
        Self::Exact(value)
    }
}

impl From<PanSpeed> for Speed<PanSpeed> {
    fn from(value: PanSpeed) -> Self {
        Self::Exact(value)
    }
}

impl From<TiltSpeed> for Speed<TiltSpeed> {
    fn from(value: TiltSpeed) -> Self {
        Self::Exact(value)
    }
}

impl From<ZoomSpeed> for Speed<ZoomSpeed> {
    fn from(value: ZoomSpeed) -> Self {
        Self::Exact(value)
    }
}

impl From<FocusSpeed> for Speed<FocusSpeed> {
    fn from(value: FocusSpeed) -> Self {
        Self::Exact(value)
    }
}

/// Exact pan and tilt speeds for a movement.
///
/// The speed wrappers validate the union domain. Preparation checks the
/// selected profile's ranges and paired-speed grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct PanTiltSpeed {
    /// Horizontal movement speed.
    pub pan: PanSpeed,
    /// Vertical movement speed.
    pub tilt: TiltSpeed,
}

impl PanTiltSpeed {
    /// Pairs checked axis speeds; profile validation occurs during preparation.
    #[must_use]
    pub const fn new(pan: PanSpeed, tilt: TiltSpeed) -> Self {
        Self { pan, tilt }
    }
}

/// Descriptive F-stop values for iris control.
///
/// The value encoding identifies an F-stop; it is not a camera's iris code.
/// The selected profile's iris table supplies any physical-to-code mapping.
#[crate::value_enum]
pub enum FStop {
    /// Iris completely closed.
    Closed = 0,
    /// F/11, a small aperture opening.
    F11 = 1,
    /// F/9.6.
    #[value(display = "F9.6")]
    F9_6 = 2,
    /// F/8.
    F8 = 3,
    /// F/6.8.
    #[value(display = "F6.8")]
    F6_8 = 4,
    /// F/5.6.
    #[value(display = "F5.6")]
    F5_6 = 5,
    /// F/4.8.
    #[value(display = "F4.8")]
    F4_8 = 6,
    /// F/4.
    F4 = 7,
    /// F/3.4.
    #[value(display = "F3.4")]
    F3_4 = 8,
    /// F/2.8.
    #[value(display = "F2.8")]
    F2_8 = 9,
    /// F/2.4.
    #[value(display = "F2.4")]
    F2_4 = 10,
    /// F/2.
    #[value(display = "F2.0")]
    F2 = 11,
    /// F/1.8, a wide aperture opening.
    #[value(display = "F1.8")]
    F1_8 = 12,
}

/// NDI streaming quality setting.
#[crate::value_enum]
pub enum NdiQuality {
    /// Highest quality and bandwidth.
    High = 0x01,
    /// Balanced quality and bandwidth.
    Medium = 0x02,
    /// Lower quality and bandwidth.
    Low = 0x03,
    /// Disable NDI streaming.
    Off = 0x04,
}

/// Zoom position code; the selected profile supplies the usable zoom domains.
#[crate::value(min = 0x0000, max = 0x7FFF, display = "Zoom {:#06x}")]
pub struct ZoomPosition(u16);

/// Focus position code, checked against the selected profile during preparation.
///
/// The union wire domain is the full four-nibble field, `0x0000..=0xFFFF`.
#[crate::value(min = 0x0000, max = 0xFFFF, display = "Focus {:#06x}")]
pub struct FocusPosition(u16);

/// Red or blue channel gain for the standard manual white-balance registers.
#[crate::value(min = 0x00, max = 0xFF, display = "RGB Gain {:#04x}")]
pub struct RgbGain(u8);

/// Saturation adjustment code.
#[crate::value(min = 0x00, max = 0x0E, display = "Saturation {}")]
pub struct SaturationLevel(u8);

/// Hue adjustment code.
#[crate::value(min = 0x00, max = 0x0E, display = "Hue {}")]
pub struct HueLevel(u8);

/// Gamma curve adjustment code.
#[crate::value(min = 0, max = 4, display = "Gamma {}")]
pub struct GammaLevel(u8);

/// Red or blue channel white-balance tuning, from -10 through +10.
///
/// The wire code is the tuning plus ten; zero is neutral.
#[crate::value(min = -10, max = 10, display = "RGB Tuning {}", wire = centered(10))]
pub struct RgbTuning(i8);

impl RgbTuning {
    /// Neutral tuning value.
    pub const NEUTRAL: Self = Self(0);
}

/// Horizontal movement speed code in the union of supported profile domains.
///
/// Preparation checks the selected profile's speed range. For profiles with
/// variable speed modes, speeds above [`PanTiltSpeedMode::STANDARD_MAXIMUM`]
/// require the fine mode. The owner checks the confirmed target state at write
/// time because a queued mode change can apply after preparation.
#[crate::value(min = 0x00, max = 0x32, display = "Pan Speed {}")]
pub struct PanSpeed(u8);

impl PanSpeed {
    /// Zero wire value. Active pan movement requires a nonzero speed.
    pub const ZERO: Self = Self(0);
}

/// Vertical movement speed code in the union of supported profile domains.
///
/// Preparation checks the selected profile's speed range. For profiles with
/// variable speed modes, speeds above [`PanTiltSpeedMode::STANDARD_MAXIMUM`]
/// require the fine mode. The owner checks the confirmed target state at write
/// time because a queued mode change can apply after preparation.
#[crate::value(min = 0x00, max = 0x32, display = "Tilt Speed {}")]
pub struct TiltSpeed(u8);

impl TiltSpeed {
    /// Zero wire value. Active tilt movement requires a nonzero speed.
    pub const ZERO: Self = Self(0);
}

/// Variable zoom speed, from zero (slowest movement) through seven (fastest).
#[crate::value(min = 0x00, max = 0x07, display = "Zoom Speed {}")]
pub struct ZoomSpeed(u8);

impl ZoomSpeed {
    /// Slowest variable movement; stopping uses the zoom STOP request.
    pub const ZERO: Self = Self(0);
}

/// Variable focus speed, from zero (slowest movement) through seven (fastest).
#[crate::value(min = 0x00, max = 0x07, display = "Focus Speed {}")]
pub struct FocusSpeed(u8);

impl FocusSpeed {
    /// Slowest variable movement; stopping uses the focus STOP request.
    pub const ZERO: Self = Self(0);
}

/// Synchronized movement speed code from one through twenty-four.
#[crate::value(min = 0x01, max = 0x18, display = "Motion Sync Speed {}")]
pub struct MotionSyncSpeed(u8);

impl MotionSyncSpeed {
    /// Slow synchronized movement.
    pub const SLOW: Self = Self(8);
    /// Normal synchronized movement.
    pub const NORMAL: Self = Self(16);
    /// Fast synchronized movement.
    pub const FAST: Self = Self(24);
}

/// Saved ND filter preset number.
#[crate::value(min = 0x00, max = 0x03, display = "ND Filter Preset {}")]
pub struct NdFilterPreset(u8);

/// Tally indication mode.
#[crate::value_enum]
pub enum TallyMode {
    /// Turn the tally off.
    Off = 0x03,
    /// Keep the tally on continuously.
    On = 0x02,
    /// Flash the tally.
    Flash = 0x01,
}

/// Tally brightness setting.
#[crate::value_enum]
pub enum TallyBrightness {
    /// Lower tally brightness.
    Low = 0x04,
    /// Higher tally brightness.
    High = 0x05,
}

#[cfg(test)]
#[path = "types/value_tests.rs"]
mod value_tests;

pub use crate::camera_id::{CameraId, IndividualCameraId};
pub use crate::requests::{AffectedAxes, AffectedAxis, AffectedAxisIter};
pub use crate::visca_socket::{CommandSocketCount, ViscaSocket};
