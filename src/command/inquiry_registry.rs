//! Unified inquiry registry.
//!
//! This module defines all VISCA inquiry types in a single location using the
//! `define_inquiries!` macro, which generates:
//! - [`InquiryKind`] enum (response type discriminator)
//! - [`InquiryData`] enum (decoded response payload)
//! - [`dispatch()`] function (maps `InquiryKind` + payload -> `InquiryData`)
//!
//! Adding a new inquiry requires a single entry here; the macro keeps all
//! three artifacts perfectly synchronized.

use std::borrow::Cow;

use super::exposure::{AntiFlickerMode, ExposureMode};
use super::focus::{AutoFocusSensitivity, FocusMode, FocusRange, FocusZone};
use super::image::{BlackWhiteMode, NoiseReductionMode, NoiseReductionSpeed, SharpnessMode};
use super::resolution::{NdFilterPosition, PictureEffectMode, ResolutionMode};
use super::response::payload::{BoolConvention, Nibbles, Nibbles4Or8, Payload};
use super::response::types::Response;
use super::system::{MotionSyncMode, MotionSyncPreset};
use super::white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode};
use crate::error::{format_payload_hex, Error};
use crate::types::{BroadcastDomain, DefogLevel, ExposureCompensationPosition, NdFilterPreset};

/// Generates [`InquiryKind`], [`InquiryData`], and [`dispatch()`] from a single
/// definition table.
///
/// Each entry has the form:
///
/// ```text
/// #[doc = "..."]
/// VariantName { field: Type, ... } => |payload| { decode_body }
/// ```
///
/// For tuple variants use `(Type)` instead of `{ field: Type }`.
macro_rules! define_inquiries {
    (
        $(
            $(#[$meta:meta])*
            $variant:ident $body:tt => |$payload:ident| $decode_body:block
        ),* $(,)?
    ) => {
        /// Type of expected response for inquiry commands.
        ///
        /// Used to indicate what kind of data parser should expect in the response
        /// payload.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum InquiryKind {
            $(
                $(#[$meta])*
                $variant,
            )*
        }

        /// Response data from VISCA inquiry commands.
        ///
        /// Each variant represents a different type of inquiry response with its
        /// associated data.  These are returned wrapped in
        /// [`Response::Inquiry(...)`](Response::Inquiry).
        #[derive(Debug, Copy, Clone)]
        pub enum InquiryData {
            $(
                $(#[$meta])*
                $variant $body,
            )*
        }

        /// Decode an inquiry response by dispatching to the appropriate decoder.
        ///
        /// Generated from the inquiry registry to ensure every [`InquiryKind`]
        /// variant has a corresponding decoder arm.
        pub(crate) fn dispatch(
            kind: InquiryKind,
            payload: Payload<'_>,
        ) -> Result<Response, Error> {
            match kind {
                $(
                    InquiryKind::$variant => {
                        let $payload = payload;
                        $decode_body
                    }
                ),*
            }
        }
    };
}

define_inquiries! {
    // ================================================================
    // Power-related
    // ================================================================

    /// Power status inquiry response.
    Power {
        /// Whether the camera is powered on.
        on: bool,
    } => |payload| {
        let on = payload.parse_bool("power_status", BoolConvention::OnIs02)?;
        Ok(Response::Inquiry(InquiryData::Power { on }))
    },

    /// Standby mode inquiry response.
    Standby {
        /// Whether the camera is in standby mode.
        in_standby: bool,
    } => |payload| {
        let in_standby = payload.parse_bool("standby_mode", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::Standby { in_standby }))
    },

    // ================================================================
    // Pan/Tilt-related
    // ================================================================

    /// Pan/Tilt position inquiry response.
    PanTiltPosition {
        /// Current pan position.
        pan: i16,
        /// Current tilt position.
        tilt: i16,
    } => |payload| {
        decode_pan_tilt_position(payload)
    },

    // ================================================================
    // Zoom-related
    // ================================================================

    /// Current zoom position inquiry response.
    ZoomPosition {
        /// Zoom position value.
        position: u16,
    } => |payload| {
        let nibbles = Nibbles4Or8::try_from(payload)?;
        if matches!(nibbles, Nibbles4Or8::N8(_)) {
            tracing::warn!(
                "ZoomPosition: Received extended format (8 nibbles). Using first 4 nibbles (16-bit) per VISCA spec."
            );
        }
        let position = nibbles.first_u16();
        Ok(Response::Inquiry(InquiryData::ZoomPosition { position }))
    },

    /// Zoom out state inquiry response.
    ZoomOut {
        /// Whether zoom out is active.
        active: bool,
    } => |payload| {
        let active = payload.parse_bool("zoom_out_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::ZoomOut { active }))
    },

    /// Zoom in state inquiry response.
    ZoomIn {
        /// Whether zoom in is active.
        active: bool,
    } => |payload| {
        let active = payload.parse_bool("zoom_in_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::ZoomIn { active }))
    },

    /// Zoom tele/wide state inquiry response.
    ZoomTeleWide {
        /// Whether zoom tele is active (false = wide active).
        tele: bool,
    } => |payload| {
        let tele = payload.parse_bool("zoom_tele_wide_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::ZoomTeleWide { tele }))
    },

    // ================================================================
    // Focus-related
    // ================================================================

    /// Current focus position inquiry response.
    FocusPosition {
        /// Focus position value.
        position: u16,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let position = nibbles.u16_quad(0);
        Ok(Response::Inquiry(InquiryData::FocusPosition { position }))
    },

    /// Focus near limit inquiry response.
    FocusNearLimit {
        /// Near limit position value.
        position: u16,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let position = nibbles.u16_quad(0);
        Ok(Response::Inquiry(InquiryData::FocusNearLimit { position }))
    },

    /// Focus zone inquiry response.
    FocusZone {
        /// Current focus zone setting.
        zone: FocusZone,
    } => |payload| {
        require_len(&payload, 1)?;
        let zone = match payload.as_slice()[0] {
            0x00 => FocusZone::Top,
            0x01 => FocusZone::Center,
            0x02 => FocusZone::Bottom,
            v => {
                return Err(Error::InvalidParameter {
                    parameter: "focus_zone",
                    value: Cow::Owned(format!("{v:02X}")),
                    reason: Cow::Borrowed("Unknown focus zone value"),
                })
            }
        };
        Ok(Response::Inquiry(InquiryData::FocusZone { zone }))
    },

    /// Auto-focus sensitivity inquiry response.
    AutoFocusSensitivity {
        /// Current auto-focus sensitivity setting.
        sensitivity: AutoFocusSensitivity,
    } => |payload| {
        require_len(&payload, 1)?;
        let sensitivity = match payload.as_slice()[0] {
            0x00 => AutoFocusSensitivity::Low,
            0x01 => AutoFocusSensitivity::Normal,
            0x02 => AutoFocusSensitivity::High,
            v => {
                return Err(Error::InvalidParameter {
                    parameter: "auto_focus_sensitivity",
                    value: Cow::Owned(format!("{v:02X}")),
                    reason: Cow::Borrowed("Unknown auto focus sensitivity value"),
                })
            }
        };
        Ok(Response::Inquiry(InquiryData::AutoFocusSensitivity { sensitivity }))
    },

    /// Focus mode inquiry response.
    FocusMode {
        /// Current focus mode (Auto or Manual).
        mode: FocusMode,
    } => |payload| {
        require_len(&payload, 1)?;
        let mode = match payload.as_slice()[0] {
            0x02 => FocusMode::Auto,
            0x03 => FocusMode::Manual,
            v => {
                return Err(Error::InvalidParameter {
                    parameter: "focus_mode",
                    value: Cow::Owned(format!("{v:02X}")),
                    reason: Cow::Borrowed("Unknown focus mode value"),
                })
            }
        };
        Ok(Response::Inquiry(InquiryData::FocusMode { mode }))
    },

    /// Focus range mode inquiry response.
    FocusRange {
        /// Current focus range setting.
        range: FocusRange,
    } => |payload| {
        require_nonempty(&payload)?;
        let range = FocusRange::try_from(payload.as_slice()[0])?;
        Ok(Response::Inquiry(InquiryData::FocusRange { range }))
    },

    /// Auto focus enable/disable status inquiry response.
    AutoFocus {
        /// Whether auto focus is enabled.
        enabled: bool,
    } => |payload| {
        let enabled = payload.parse_bool("autofocus_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::AutoFocus { enabled }))
    },

    /// Focus unlock state inquiry response.
    FocusUnlock {
        /// Whether focus is unlocked.
        unlocked: bool,
    } => |payload| {
        let unlocked = payload.parse_bool("focus_unlock", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::FocusUnlock { unlocked }))
    },

    /// Focus near/far state inquiry response.
    FocusNearFar {
        /// Whether focus near is active (false = far active).
        near: bool,
    } => |payload| {
        let near = payload.parse_bool("focus_near_far_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::FocusNearFar { near }))
    },

    // ================================================================
    // Exposure-related
    // ================================================================

    /// Exposure mode inquiry response.
    ExposureMode {
        /// Current exposure mode (Auto, Manual, Shutter, Iris, or Bright).
        mode: ExposureMode,
    } => |payload| {
        require_len(&payload, 1)?;
        let mode = match payload.as_slice()[0] {
            0x00 => ExposureMode::Auto,
            0x03 => ExposureMode::Manual,
            0x0A => ExposureMode::Shutter,
            0x0B => ExposureMode::Iris,
            0x0D => ExposureMode::Bright,
            v => {
                return Err(Error::InvalidParameter {
                    parameter: "exposure_mode",
                    value: Cow::Owned(format!("{v:02X}")),
                    reason: Cow::Borrowed("Unknown exposure mode value"),
                })
            }
        };
        Ok(Response::Inquiry(InquiryData::ExposureMode { mode }))
    },

    /// Exposure compensation mode inquiry response.
    ExposureCompensationMode {
        /// Whether exposure compensation is enabled.
        on: bool,
    } => |payload| {
        let on = payload.parse_bool("exposure_compensation_mode", BoolConvention::OnIs02)?;
        Ok(Response::Inquiry(InquiryData::ExposureCompensationMode { on }))
    },

    /// Exposure compensation inquiry response.
    ExposureCompensation {
        /// Exposure compensation value (-7 to +7).
        value: i8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let raw_value = nibbles.u8_pair(2);
        #[allow(clippy::cast_possible_wrap)]
        let value = raw_value as i8 - 7;
        Ok(Response::Inquiry(InquiryData::ExposureCompensation { value }))
    },

    /// Exposure compensation position inquiry response.
    ExposureCompensationPosition {
        /// Exposure compensation position value (high-resolution EV adjustment).
        position: ExposureCompensationPosition,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let position = ExposureCompensationPosition::new(nibbles.u16_quad(0));
        Ok(Response::Inquiry(InquiryData::ExposureCompensationPosition { position }))
    },

    /// Shutter speed inquiry response.
    Shutter {
        /// Shutter position (0x01=1/30 to 0x11=1/10000).
        position: u16,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let position = nibbles.u8_pair(2) as u16;
        Ok(Response::Inquiry(InquiryData::Shutter { position }))
    },

    /// Iris position inquiry response.
    Iris {
        /// Iris position (0x0=Close to 0xC=F1.8).
        position: u8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        Ok(Response::Inquiry(InquiryData::Iris { position: nibbles.last_nibble() }))
    },

    /// Brightness inquiry response.
    Brightness {
        /// Brightness position (0x00=0 to 0x11=17).
        position: u16,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let position = nibbles.u16_quad(0);
        Ok(Response::Inquiry(InquiryData::Brightness { position }))
    },

    /// Gain level inquiry response.
    Gain {
        /// Gain level value (0x00=0 to 0x07=7).
        gain: u8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        Ok(Response::Inquiry(InquiryData::Gain { gain: nibbles.last_nibble() }))
    },

    /// Gain limit inquiry response.
    GainLimit {
        /// Maximum gain limit (0x0=0 to 0xF=15).
        limit: u8,
    } => |payload| {
        require_len(&payload, 1)?;
        Ok(Response::Inquiry(InquiryData::GainLimit { limit: payload.as_slice()[0] }))
    },

    /// Iris control mode inquiry response.
    IrisControl {
        /// Whether iris is in auto mode.
        auto: bool,
    } => |payload| {
        let auto = payload.parse_bool("iris_control", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::IrisControl { auto }))
    },

    /// Iris up state inquiry response.
    IrisUp {
        /// Whether iris up is active.
        active: bool,
    } => |payload| {
        let active = payload.parse_bool("iris_up_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::IrisUp { active }))
    },

    /// Iris down state inquiry response.
    IrisDown {
        /// Whether iris down is active.
        active: bool,
    } => |payload| {
        let active = payload.parse_bool("iris_down_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::IrisDown { active }))
    },

    /// Backlight compensation inquiry response.
    Backlight {
        /// Whether backlight compensation is enabled.
        status: bool,
    } => |payload| {
        let status = payload.parse_bool("backlight_status", BoolConvention::OnIs02)?;
        Ok(Response::Inquiry(InquiryData::Backlight { status }))
    },

    /// Dynamic range control inquiry response.
    DynamicRange {
        /// Dynamic range level (0x0=0 to 0x8=8).
        level: u8,
    } => |payload| {
        require_len(&payload, 1)?;
        let level = payload.as_slice()[0];
        Ok(Response::Inquiry(InquiryData::DynamicRange { level }))
    },

    // ================================================================
    // Color-related
    // ================================================================

    /// White balance mode inquiry response.
    WhiteBalanceMode {
        /// Current white balance mode.
        mode: WhiteBalanceMode,
    } => |payload| {
        require_len(&payload, 1)?;
        let mode = match payload.as_slice()[0] {
            0x00 => WhiteBalanceMode::Auto,
            0x01 => WhiteBalanceMode::Indoor,
            0x02 => WhiteBalanceMode::Outdoor,
            0x03 => WhiteBalanceMode::OnePush,
            0x05 => WhiteBalanceMode::Manual,
            0x20 => WhiteBalanceMode::ColorTemperature,
            v => {
                return Err(Error::InvalidParameter {
                    parameter: "white_balance_mode",
                    value: Cow::Owned(format!("{v:02X}")),
                    reason: Cow::Borrowed("Unknown white balance mode value"),
                })
            }
        };
        Ok(Response::Inquiry(InquiryData::WhiteBalanceMode { mode }))
    },

    /// Color temperature inquiry response.
    ColorTemperature {
        /// Color temperature in Kelvin.
        temperature: u16,
    } => |payload| {
        require_len(&payload, 1)?;
        let temperature = payload.as_slice()[0] as u16;
        Ok(Response::Inquiry(InquiryData::ColorTemperature { temperature }))
    },

    /// Red channel gain inquiry response.
    RedChannel {
        /// Red channel absolute gain value.
        gain: u8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let gain = nibbles.u8_pair(2);
        Ok(Response::Inquiry(InquiryData::RedChannel { gain }))
    },

    /// Blue channel gain inquiry response.
    BlueChannel {
        /// Blue channel absolute gain value.
        gain: u8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let gain = nibbles.u8_pair(2);
        Ok(Response::Inquiry(InquiryData::BlueChannel { gain }))
    },

    /// Red channel tuning inquiry response.
    RedTuning {
        /// Red channel tuning level (-10 to +10).
        level: i8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let raw = nibbles.u8_pair(2);
        #[allow(clippy::cast_possible_wrap)]
        let level = raw as i8 - 10;
        Ok(Response::Inquiry(InquiryData::RedTuning { level }))
    },

    /// Blue channel tuning inquiry response.
    BlueTuning {
        /// Blue channel tuning level (-10 to +10).
        level: i8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let raw = nibbles.u8_pair(2);
        #[allow(clippy::cast_possible_wrap)]
        let level = raw as i8 - 10;
        Ok(Response::Inquiry(InquiryData::BlueTuning { level }))
    },

    /// Auto white balance sensitivity inquiry response.
    AutoWhiteBalanceSensitivity {
        /// Sensitivity level (Low, Normal, High).
        sensitivity: AutoWhiteBalanceSensitivity,
    } => |payload| {
        require_nonempty(&payload)?;
        let sensitivity = match payload.as_slice()[0] {
            0x00 => AutoWhiteBalanceSensitivity::High,
            0x01 => AutoWhiteBalanceSensitivity::Normal,
            0x02 => AutoWhiteBalanceSensitivity::Low,
            v => {
                return Err(Error::InvalidParameter {
                    parameter: "auto_wb_sensitivity",
                    value: Cow::Owned(format!("0x{v:02X}")),
                    reason: Cow::Borrowed(
                        "Invalid auto white balance sensitivity. Expected 0x00 (High), 0x01 (Normal), or 0x02 (Low)",
                    ),
                })
            }
        };
        Ok(Response::Inquiry(InquiryData::AutoWhiteBalanceSensitivity { sensitivity }))
    },

    // ================================================================
    // Image-related
    // ================================================================

    /// Luminance level inquiry response.
    Luminance {
        /// Current luminance (image processing brightness) level.
        level: u8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        Ok(Response::Inquiry(InquiryData::Luminance { level: nibbles.last_nibble() }))
    },

    /// Contrast level inquiry response.
    Contrast {
        /// Current contrast level.
        level: u8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        Ok(Response::Inquiry(InquiryData::Contrast { level: nibbles.last_nibble() }))
    },

    /// Sharpness value inquiry response.
    Sharpness {
        /// Current sharpness value.
        value: u8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let value = nibbles.u8_pair(2);
        Ok(Response::Inquiry(InquiryData::Sharpness { value }))
    },

    /// Sharpness position inquiry response.
    SharpnessPosition {
        /// Current sharpness position value.
        position: u16,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        let position = nibbles.u16_quad(0);
        Ok(Response::Inquiry(InquiryData::SharpnessPosition { position }))
    },

    /// Sharpness mode inquiry response.
    SharpnessMode {
        /// Current sharpness mode.
        mode: SharpnessMode,
    } => |payload| {
        require_nonempty(&payload)?;
        let mode = match payload.as_slice()[0] {
            0x02 => SharpnessMode::Auto,
            0x03 => SharpnessMode::Manual,
            v => {
                return Err(Error::InvalidParameter {
                    parameter: "sharpness_mode",
                    value: Cow::Owned(format!("0x{v:02X}")),
                    reason: Cow::Borrowed("Expected 0x02 (Auto) or 0x03 (Manual)"),
                })
            }
        };
        Ok(Response::Inquiry(InquiryData::SharpnessMode { mode }))
    },

    /// Color saturation inquiry response.
    Saturation {
        /// Saturation level (0x0=60% to 0xE=200%).
        level: u8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        Ok(Response::Inquiry(InquiryData::Saturation { level: nibbles.last_nibble() }))
    },

    /// Color hue inquiry response.
    Hue {
        /// Hue value (0x0=0 to 0xE=14).
        hue: u8,
    } => |payload| {
        let nibbles = Nibbles::<4>::try_from(payload)?;
        Ok(Response::Inquiry(InquiryData::Hue { hue: nibbles.last_nibble() }))
    },

    /// Picture effect mode inquiry response.
    PictureEffect {
        /// Current picture effect (Off, Negative, Black & White, Sepia, etc.).
        effect: PictureEffectMode,
    } => |payload| {
        require_nonempty(&payload)?;
        Ok(Response::Inquiry(InquiryData::PictureEffect {
            effect: PictureEffectMode::from_byte(payload.as_slice()[0]),
        }))
    },

    /// Black and white mode inquiry response.
    BlackWhite {
        /// Whether black and white mode is enabled.
        on: bool,
    } => |payload| {
        require_len(&payload, 1)?;
        Ok(Response::Inquiry(InquiryData::BlackWhite { on: payload.as_slice()[0] == 0x04 }))
    },

    /// Black and white mode state inquiry response.
    BlackWhiteMode {
        /// Current black and white mode setting.
        mode: BlackWhiteMode,
    } => |payload| {
        require_len(&payload, 1)?;
        let mode = BlackWhiteMode::try_from(payload.as_slice()[0])?;
        Ok(Response::Inquiry(InquiryData::BlackWhiteMode { mode }))
    },

    /// 2D noise reduction inquiry response.
    NoiseReduction2D {
        /// 2D noise reduction level.
        level: u8,
    } => |payload| {
        require_len(&payload, 1)?;
        let level = payload.as_slice()[0];
        Ok(Response::Inquiry(InquiryData::NoiseReduction2D { level }))
    },

    /// 3D noise reduction inquiry response.
    NoiseReduction3D {
        /// 3D noise reduction level.
        level: u8,
    } => |payload| {
        require_len(&payload, 1)?;
        let level = payload.as_slice()[0];
        Ok(Response::Inquiry(InquiryData::NoiseReduction3D { level }))
    },

    /// Noise reduction mode inquiry response.
    NoiseReductionMode {
        /// Current noise reduction mode setting.
        mode: NoiseReductionMode,
    } => |payload| {
        require_len(&payload, 1)?;
        let mode = NoiseReductionMode::try_from(payload.as_slice()[0])?;
        Ok(Response::Inquiry(InquiryData::NoiseReductionMode { mode }))
    },

    /// Noise reduction speed inquiry response.
    NoiseReductionSpeed {
        /// Current noise reduction speed setting.
        speed: NoiseReductionSpeed,
    } => |payload| {
        require_len(&payload, 1)?;
        let speed = NoiseReductionSpeed::try_from(payload.as_slice()[0])?;
        Ok(Response::Inquiry(InquiryData::NoiseReductionSpeed { speed }))
    },

    /// Noise reduction level inquiry response.
    NoiseReductionLevel(u8) => |payload| {
        require_len(&payload, 1)?;
        Ok(Response::Inquiry(InquiryData::NoiseReductionLevel(payload.as_slice()[0])))
    },

    /// Image flip inquiry response.
    FlipState {
        /// Whether horizontal flip is enabled.
        horizontal: bool,
        /// Whether vertical flip is enabled.
        vertical: bool,
    } => |payload| {
        require_nonempty(&payload)?;
        let mode = payload.as_slice()[0];
        let horizontal = (mode & 0x01) != 0;
        let vertical = (mode & 0x02) != 0;
        Ok(Response::Inquiry(InquiryData::FlipState { horizontal, vertical }))
    },

    /// Two tone mode inquiry response.
    TwoToneMode {
        /// Whether two tone mode is enabled.
        on: bool,
    } => |payload| {
        let on = payload.parse_bool("two_tone_mode_status", BoolConvention::OnIs02)?;
        Ok(Response::Inquiry(InquiryData::TwoToneMode { on }))
    },

    /// Defog mode inquiry response.
    DefogMode {
        /// Whether defog is enabled.
        enabled: bool,
    } => |payload| {
        let enabled = payload.parse_bool("defog_mode", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::DefogMode { enabled }))
    },

    /// Defog level inquiry response.
    DefogLevel {
        /// Current defog strength level (0-5).
        level: DefogLevel,
    } => |payload| {
        require_nonempty(&payload)?;
        let level = DefogLevel::new(payload.as_slice()[0]).map_err(|_| Error::InvalidParameter {
            parameter: "defog_level",
            value: Cow::Owned(payload.as_slice()[0].to_string()),
            reason: Cow::Borrowed("value out of range (0-5)"),
        })?;
        Ok(Response::Inquiry(InquiryData::DefogLevel { level }))
    },

    /// Gamma curve setting inquiry response.
    Gamma {
        /// Gamma curve setting (0=Standard, 1-4=different gamma curves).
        value: u8,
    } => |payload| {
        require_nonempty(&payload)?;
        Ok(Response::Inquiry(InquiryData::Gamma { value: payload.as_slice()[0] }))
    },

    /// ND filter state inquiry response.
    NdFilter {
        /// Current ND filter position (Clear, 1/4, 1/8, 1/16, etc.).
        position: NdFilterPosition,
    } => |payload| {
        require_nonempty(&payload)?;
        Ok(Response::Inquiry(InquiryData::NdFilter {
            position: NdFilterPosition::from_byte(payload.as_slice()[0]),
        }))
    },

    /// ND filter preset inquiry response.
    NdFilterPreset {
        /// Current ND filter preset number.
        preset: NdFilterPreset,
    } => |payload| {
        require_len(&payload, 1)?;
        let preset = NdFilterPreset::new(payload.as_slice()[0]).map_err(|_| {
            Error::InvalidParameter {
                parameter: "nd_filter_preset",
                value: Cow::Owned(payload.as_slice()[0].to_string()),
                reason: Cow::Borrowed("value out of range (0-3)"),
            }
        })?;
        Ok(Response::Inquiry(InquiryData::NdFilterPreset { preset }))
    },

    // ================================================================
    // System-related
    // ================================================================

    /// Camera version information inquiry response.
    Version {
        /// Vendor ID.
        vendor: u16,
        /// Model ID.
        model: u16,
        /// ROM version.
        rom_version: u32,
        /// Maximum socket number.
        max_socket: u8,
    } => |payload| {
        if payload.len() != 7 {
            return Err(Error::invalid_response_length(7, payload.as_slice()));
        }
        let vendor = ((payload.as_slice()[0] as u16) << 8) | (payload.as_slice()[1] as u16);
        let model = ((payload.as_slice()[2] as u16) << 8) | (payload.as_slice()[3] as u16);
        let rom_version = ((payload.as_slice()[4] as u32) << 8) | (payload.as_slice()[5] as u32);
        let max_socket = payload.as_slice()[6];
        Ok(Response::Inquiry(InquiryData::Version { vendor, model, rom_version, max_socket }))
    },

    /// Video resolution inquiry response.
    Resolution(ResolutionMode) => |payload| {
        require_len(&payload, 1)?;
        let resolution_mode = ResolutionMode::from_byte(payload.as_slice()[0]);
        Ok(Response::Inquiry(InquiryData::Resolution(resolution_mode)))
    },

    /// Menu open/close status inquiry response.
    MenuOpenClose {
        /// Whether the camera menu is open.
        is_open: bool,
    } => |payload| {
        let is_open = payload.parse_bool("menu_status", BoolConvention::OnIs02)?;
        Ok(Response::Inquiry(InquiryData::MenuOpenClose { is_open }))
    },

    /// USB audio state inquiry response.
    UsbAudio {
        /// Whether USB audio is enabled.
        on: bool,
    } => |payload| {
        let on = payload.parse_bool("usb_audio_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::UsbAudio { on }))
    },

    /// RTMP state inquiry response.
    Rtmp {
        /// Whether RTMP streaming is enabled.
        on: bool,
    } => |payload| {
        let on = payload.parse_bool("rtmp_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::Rtmp { on }))
    },

    /// Digital mode inquiry response.
    Digital {
        /// Whether digital mode is enabled.
        on: bool,
    } => |payload| {
        let on = payload.parse_bool("digital_mode_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::Digital { on }))
    },

    /// Digital Ptz mode inquiry response.
    DigitalPtz {
        /// Whether digital Ptz is enabled.
        enabled: bool,
    } => |payload| {
        let enabled = payload.parse_bool("digital_ptz", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::DigitalPtz { enabled }))
    },

    /// Broadcast domain inquiry response.
    BroadcastDomain(BroadcastDomain) => |payload| {
        require_len(&payload, 1)?;
        let domain = BroadcastDomain::new(payload.as_slice()[0]).map_err(|_| {
            Error::InvalidParameter {
                parameter: "broadcast_domain",
                value: Cow::Owned(payload.as_slice()[0].to_string()),
                reason: Cow::Borrowed("value out of range (0-3)"),
            }
        })?;
        Ok(Response::Inquiry(InquiryData::BroadcastDomain(domain)))
    },

    /// Motion sync mode inquiry response.
    MotionSyncMode {
        /// Current motion sync mode setting.
        mode: MotionSyncMode,
    } => |payload| {
        require_len(&payload, 1)?;
        let mode = MotionSyncMode::try_from(payload.as_slice()[0])?;
        Ok(Response::Inquiry(InquiryData::MotionSyncMode { mode }))
    },

    /// Motion sync speed inquiry response.
    MotionSyncPreset {
        /// Current motion sync speed setting.
        speed: MotionSyncPreset,
    } => |payload| {
        require_len(&payload, 1)?;
        let speed = MotionSyncPreset::try_from(payload.as_slice()[0])?;
        Ok(Response::Inquiry(InquiryData::MotionSyncPreset { speed }))
    },

    // ================================================================
    // Night/Day-related
    // ================================================================

    /// Night/Day mode inquiry response.
    NightDayMode {
        /// Whether the camera is in night mode.
        is_night: bool,
    } => |payload| {
        let is_night = payload.parse_bool("night_day_mode", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::NightDayMode { is_night }))
    },

    /// Night/day position inquiry response.
    NightDayPosition {
        /// Current night/day position value.
        position: u8,
    } => |payload| {
        require_len(&payload, 1)?;
        Ok(Response::Inquiry(InquiryData::NightDayPosition { position: payload.as_slice()[0] }))
    },

    /// Night/day switch inquiry response.
    NightDaySwitch {
        /// Whether night/day switch is enabled.
        enabled: bool,
    } => |payload| {
        let enabled = payload.parse_bool("night_day_switch", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::NightDaySwitch { enabled }))
    },

    /// Auto trace mode inquiry response.
    AutoTrace {
        /// Whether auto trace is enabled.
        enabled: bool,
    } => |payload| {
        let enabled = payload.parse_bool("auto_trace", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::AutoTrace { enabled }))
    },

    // ================================================================
    // Tally-related
    // ================================================================

    /// Red tally light state inquiry response.
    TallyRed {
        /// Whether the red tally light is on.
        on: bool,
    } => |payload| {
        let on = payload.parse_bool("tally_red_status", BoolConvention::OnIs02)?;
        Ok(Response::Inquiry(InquiryData::TallyRed { on }))
    },

    /// Green tally light state inquiry response.
    TallyGreen {
        /// Whether the green tally light is on.
        on: bool,
    } => |payload| {
        let on = payload.parse_bool("tally_green_status", BoolConvention::OnIs02)?;
        Ok(Response::Inquiry(InquiryData::TallyGreen { on }))
    },

    /// Tally light status inquiry response.
    TallyStatus {
        /// Whether the red tally light is on.
        red_on: bool,
        /// Whether the green tally light is on.
        green_on: bool,
    } => |payload| {
        if payload.len() < 2 {
            return Err(Error::invalid_response_length(2, payload.as_slice()));
        }
        let red_payload = Payload::new(&payload.as_slice()[0..1]);
        let green_payload = Payload::new(&payload.as_slice()[1..2]);
        let red_on = red_payload.parse_bool("tally_red_status", BoolConvention::OnIs03)?;
        let green_on = green_payload.parse_bool("tally_green_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::TallyStatus { red_on, green_on }))
    },

    /// Tally auto adjust inquiry response.
    TallyAutoAdjust {
        /// Whether tally auto adjust is enabled.
        on: bool,
    } => |payload| {
        let on = payload.parse_bool("tally_auto_adjust_status", BoolConvention::OnIs03)?;
        Ok(Response::Inquiry(InquiryData::TallyAutoAdjust { on }))
    },

    // ================================================================
    // Flicker mode
    // ================================================================

    /// Flicker mode inquiry response.
    FlickerMode {
        /// Current anti-flicker mode setting.
        mode: AntiFlickerMode,
    } => |payload| {
        require_len(&payload, 1)?;
        let mode = match payload.as_slice()[0] {
            0x00 => AntiFlickerMode::Off,
            0x01 => AntiFlickerMode::Hz50,
            0x02 => AntiFlickerMode::Hz60,
            v => {
                return Err(Error::InvalidParameter {
                    parameter: "flicker_mode",
                    value: Cow::Owned(format!("{v:02X}")),
                    reason: Cow::Borrowed("Unknown flicker mode value"),
                });
            }
        };
        Ok(Response::Inquiry(InquiryData::FlickerMode { mode }))
    }
}

// ============================================================
// Helper functions
// ============================================================

/// Require payload to have exactly the specified length.
#[inline]
fn require_len(payload: &Payload<'_>, expected: usize) -> Result<(), Error> {
    if payload.len() != expected {
        return Err(Error::invalid_response_length(expected, payload.as_slice()));
    }
    Ok(())
}

/// Require payload to be non-empty.
#[inline]
fn require_nonempty(payload: &Payload<'_>) -> Result<(), Error> {
    if payload.is_empty() {
        return Err(Error::invalid_response_length(1, payload.as_slice()));
    }
    Ok(())
}

/// Decode PanTiltPosition without profile awareness.
///
/// This decoder interprets pan/tilt positions as signed 16-bit values.
fn decode_pan_tilt_position(payload: Payload<'_>) -> Result<Response, Error> {
    if payload.len() == 8 {
        let nibbles = Nibbles::<8>::try_from(payload)?;
        let pan = nibbles.i16_quad(0);
        let tilt = nibbles.i16_quad(4);
        Ok(Response::Inquiry(InquiryData::PanTiltPosition {
            pan,
            tilt,
        }))
    } else if payload.len() == 4 {
        tracing::warn!(
            "PanTiltPosition: Received compact format (4 bytes). Payload: {:02X?}. Treating as home position.",
            payload.as_slice()
        );
        let pan = if payload.len() >= 2 {
            #[allow(clippy::cast_possible_wrap)]
            let p = ((payload.as_slice()[0] as i16) << 8) | (payload.as_slice()[1] as i16);
            p
        } else {
            0
        };
        let tilt = if payload.len() >= 4 {
            #[allow(clippy::cast_possible_wrap)]
            let t = ((payload.as_slice()[2] as i16) << 8) | (payload.as_slice()[3] as i16);
            t
        } else {
            0
        };
        Ok(Response::Inquiry(InquiryData::PanTiltPosition {
            pan,
            tilt,
        }))
    } else {
        tracing::debug!(
            "PanTiltPosition: Payload length {} doesn't match pan/tilt format (expected 8 or 4 bytes)",
            payload.len()
        );
        Err(Error::DecoderNotFound {
            inquiry_kind: InquiryKind::PanTiltPosition,
            payload_hex: format_payload_hex(payload.as_slice()),
        })
    }
}
