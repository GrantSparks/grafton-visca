//! Camera profile implementations using capability traits.
//!
//! This module contains camera profiles composed from capability traits,
//! enabling compile-time feature detection and type-safe operations.

use std::{fmt, time::Duration};

use crate::{
    capabilities::{
        CoordinateSystem, Exposure, Focus, ImageProcessing, MenuCapability, MotionSync, NdFilter,
        NdFilterMode, PanTilt, Power, Presets, ProfileMetadata, ShutterSpeed, VariableSpeed,
        WhiteBalance, Zoom,
    },
    error::Error,
    transport::envelope::{RawVisca, SonyEncapsulated},
    WhiteBalanceMode,
};

mod exposure_constants {
    use crate::capabilities::ShutterSpeed;

    pub const PTZ_OPTICS_G2_SHUTTER_SPEEDS: &[ShutterSpeed] = &[
        ShutterSpeed::new("1/30", 0x01),
        ShutterSpeed::new("1/60", 0x02),
        ShutterSpeed::new("1/90", 0x03),
        ShutterSpeed::new("1/100", 0x04),
        ShutterSpeed::new("1/125", 0x05),
        ShutterSpeed::new("1/180", 0x06),
        ShutterSpeed::new("1/250", 0x07),
        ShutterSpeed::new("1/350", 0x08),
        ShutterSpeed::new("1/500", 0x09),
        ShutterSpeed::new("1/725", 0x0A),
        ShutterSpeed::new("1/1000", 0x0B),
        ShutterSpeed::new("1/1500", 0x0C),
        ShutterSpeed::new("1/2000", 0x0D),
        ShutterSpeed::new("1/3000", 0x0E),
        ShutterSpeed::new("1/4000", 0x0F),
        ShutterSpeed::new("1/6000", 0x10),
        ShutterSpeed::new("1/10000", 0x11),
    ];

    pub const GENERIC_VISCA_SHUTTER_SPEEDS: &[ShutterSpeed] = &[
        ShutterSpeed::new("1/30", 0x00),
        ShutterSpeed::new("1/60", 0x01),
        ShutterSpeed::new("1/100", 0x02),
        ShutterSpeed::new("1/250", 0x03),
        ShutterSpeed::new("1/500", 0x04),
        ShutterSpeed::new("1/1000", 0x05),
        ShutterSpeed::new("1/2000", 0x06),
        ShutterSpeed::new("1/4000", 0x07),
        ShutterSpeed::new("1/10000", 0x08),
    ];
}

use self::exposure_constants::{GENERIC_VISCA_SHUTTER_SPEEDS, PTZ_OPTICS_G2_SHUTTER_SPEEDS};

const PTZ_OPTICS_G2_WB_MODES: &[WhiteBalanceMode] = &[
    WhiteBalanceMode::Auto,
    WhiteBalanceMode::Indoor,
    WhiteBalanceMode::Outdoor,
    WhiteBalanceMode::OnePush,
    WhiteBalanceMode::Manual,
];

const GENERIC_WB_MODES: &[WhiteBalanceMode] = &[
    WhiteBalanceMode::Auto,
    WhiteBalanceMode::Indoor,
    WhiteBalanceMode::Outdoor,
    WhiteBalanceMode::OnePush,
    WhiteBalanceMode::Manual,
];

const SONY_BRC_WB_MODES: &[WhiteBalanceMode] = &[
    WhiteBalanceMode::Auto,
    WhiteBalanceMode::Indoor,
    WhiteBalanceMode::Outdoor,
    WhiteBalanceMode::OnePush,
    WhiteBalanceMode::Manual,
];

/// PtzOptics G2 camera profile.
///
/// This camera supports:
/// - Pan/Tilt with 340° pan range and -30° to +90° tilt
/// - 20x optical zoom with digital zoom extension
/// - Auto and manual focus
/// - Full exposure control
/// - White balance with 6 modes
/// - Image processing including flip/mirror
/// - 90 preset positions
/// - Power control with standby
///
/// Does NOT support:
/// - ND filters
#[derive(Debug, Default, Clone, Copy)]
pub struct PtzOpticsG2;

impl ProfileMetadata for PtzOpticsG2 {
    const MODEL_NAME: &'static str = "PtzOptics G2";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const SUPPORTS_OPERATION_COMPLETE: bool = true;
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for PtzOpticsG2 {
    const PAN_RANGE: std::ops::Range<i16> = -2448..2449;
    const TILT_RANGE: std::ops::Range<i16> = -432..1297;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 20;
    const PAN_DEGREES_TO_UNITS: f32 = 14.4;
    const TILT_DEGREES_TO_UNITS: f32 = 14.4;
}

impl Zoom for PtzOpticsG2 {
    const OPTICAL_ZOOM_MAX: u16 = 0x4000;
    const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 862.3;
}

impl Focus for PtzOpticsG2 {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xF000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
}

impl Exposure for PtzOpticsG2 {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1D;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = PTZ_OPTICS_G2_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..9;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
}

impl WhiteBalance for PtzOpticsG2 {
    const WB_MODES: &'static [WhiteBalanceMode] = PTZ_OPTICS_G2_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
}

impl ImageProcessing for PtzOpticsG2 {
    const BRIGHTNESS_RANGE: std::ops::Range<u8> = 0..18;
    const CONTRAST_RANGE: std::ops::Range<u8> = 0..15;
    const SHARPNESS_RANGE: std::ops::Range<u8> = 0..15;
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SUPPORTS_FLIP: bool = true;
    const SUPPORTS_MIRROR: bool = true;
    const SUPPORTS_NOISE_REDUCTION: bool = true;
    const SUPPORTS_2D_NR: bool = true;
    const SUPPORTS_3D_NR: bool = true;
    const SUPPORTS_PICTURE_EFFECT: bool = false;
}

impl Presets for PtzOpticsG2 {
    const MAX_PRESETS: u8 = 89;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..25;
    const SUPPORTS_PRESET_TOUR: bool = false;
}

impl Power for PtzOpticsG2 {
    const POWER_ON_TIME: Duration = Duration::from_secs(10);
    const SUPPORTS_STANDBY: bool = true;
}

impl MotionSync for PtzOpticsG2 {
    const SUPPORTS_MOTION_SYNC: bool = true;
    const MAX_MOTION_SYNC_SPEED: u8 = 24;
}
impl MenuCapability for PtzOpticsG2 {}
impl crate::capabilities::Tally for PtzOpticsG2 {}

impl crate::capabilities::HasAutoExposure for PtzOpticsG2 {}
impl crate::capabilities::HasBacklightCompensation for PtzOpticsG2 {}
impl crate::capabilities::HasWDR for PtzOpticsG2 {}
impl crate::capabilities::HasOnePushWhiteBalance for PtzOpticsG2 {}
impl crate::capabilities::HasAutoFocus for PtzOpticsG2 {}
impl crate::capabilities::HasOnePushFocus for PtzOpticsG2 {}
impl crate::capabilities::HasFocusLock for PtzOpticsG2 {}

/// Generic VISCA camera profile.
///
/// Conservative profile for unknown VISCA cameras with basic features only.
#[derive(Debug, Default, Clone, Copy)]
pub struct GenericVisca;

impl ProfileMetadata for GenericVisca {
    const MODEL_NAME: &'static str = "Generic VISCA Camera";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(200);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(10000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for GenericVisca {
    const PAN_RANGE: std::ops::Range<i16> = -2880..2881;
    const TILT_RANGE: std::ops::Range<i16> = -1440..1441;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 24;
    const PAN_DEGREES_TO_UNITS: f32 = 16.0;
    const TILT_DEGREES_TO_UNITS: f32 = 16.0;
}

impl Zoom for GenericVisca {
    const OPTICAL_ZOOM_MAX: u16 = 0xFFFF;
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const SUPPORTS_DIRECT_ZOOM: bool = false;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 1000.0;
}

impl Power for GenericVisca {
    const POWER_ON_TIME: Duration = Duration::from_secs(30);
    const SUPPORTS_STANDBY: bool = false;
}
impl MenuCapability for GenericVisca {}

impl Exposure for GenericVisca {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1C;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = GENERIC_VISCA_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..8;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = false;
    const SUPPORTS_WDR: bool = false;
    const SUPPORTS_EXPOSURE_COMP: bool = false;
}

impl WhiteBalance for GenericVisca {
    const WB_MODES: &'static [WhiteBalanceMode] = GENERIC_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = None;
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = None;
    const SUPPORTS_COLOR_TEMP: bool = false;
    const COLOR_TEMP_RANGE: Option<std::ops::Range<u16>> = None;
}

impl Focus for GenericVisca {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xE000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
    const SUPPORTS_FOCUS_ZONE: bool = false;
    const SUPPORTS_AF_SENSITIVITY: bool = false;
}

impl ImageProcessing for GenericVisca {
    const BRIGHTNESS_RANGE: std::ops::Range<u8> = 0..15;
    const CONTRAST_RANGE: std::ops::Range<u8> = 0..15;
    const SHARPNESS_RANGE: std::ops::Range<u8> = 0..15;
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = None;
    const SUPPORTS_FLIP: bool = false;
    const SUPPORTS_MIRROR: bool = false;
    const SUPPORTS_HUE: bool = false;
    const HUE_RANGE: Option<std::ops::Range<u8>> = None;
    const SUPPORTS_NOISE_REDUCTION: bool = false;
    const SUPPORTS_2D_NR: bool = false;
    const SUPPORTS_3D_NR: bool = false;
}

impl Presets for GenericVisca {
    const MAX_PRESETS: u8 = 6;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..24;
    const SUPPORTS_PRESET_TOUR: bool = false;
    const SUPPORTS_PRESET_THUMBNAIL: bool = false;
}

impl crate::capabilities::Tally for GenericVisca {}

impl crate::capabilities::HasAutoExposure for GenericVisca {}
impl crate::capabilities::HasOnePushWhiteBalance for GenericVisca {}
impl crate::capabilities::HasAutoFocus for GenericVisca {}

/// Sony FR7 camera profile (example with ND filter).
///
/// Professional camera with all features including variable ND filter.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonyFR7;

impl ProfileMetadata for SonyFR7 {
    const MODEL_NAME: &'static str = "Sony FR7";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = SonyEncapsulated;
    const ACK_TIMEOUT: Duration = Duration::from_millis(200);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(8000);
    const BUSY_TIMEOUT: Duration = Duration::from_millis(240);
    const DEFAULT_TCP_PORT: u16 = 52381;
    const DEFAULT_UDP_PORT: u16 = 52381;
}

impl PanTilt for SonyFR7 {
    const PAN_RANGE: std::ops::Range<i16> = -2700..2701;
    const TILT_RANGE: std::ops::Range<i16> = -300..1201;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 24;
    const PAN_DEGREES_TO_UNITS: f32 = 15.88;
    const TILT_DEGREES_TO_UNITS: f32 = 15.0;
}

impl Zoom for SonyFR7 {
    const OPTICAL_ZOOM_MAX: u16 = 0x4000;
    const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 1000.0;
}

impl Focus for SonyFR7 {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xF000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
    const SUPPORTS_FOCUS_ZONE: bool = true;
    const SUPPORTS_AF_SENSITIVITY: bool = true;
}

impl Exposure for SonyFR7 {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1F;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = PTZ_OPTICS_G2_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..16;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
    const SUPPORTS_EXPOSURE_COMP: bool = true;
}

impl WhiteBalance for SonyFR7 {
    const WB_MODES: &'static [WhiteBalanceMode] = PTZ_OPTICS_G2_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const SUPPORTS_COLOR_TEMP: bool = true;
    const COLOR_TEMP_RANGE: Option<std::ops::Range<u16>> = Some(2800..7500);
}

impl ImageProcessing for SonyFR7 {
    const BRIGHTNESS_RANGE: std::ops::Range<u8> = 0..18;
    const CONTRAST_RANGE: std::ops::Range<u8> = 0..15;
    const SHARPNESS_RANGE: std::ops::Range<u8> = 0..15;
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SUPPORTS_FLIP: bool = true;
    const SUPPORTS_MIRROR: bool = true;
    const SUPPORTS_HUE: bool = true;
    const HUE_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SUPPORTS_NOISE_REDUCTION: bool = true;
    const SUPPORTS_2D_NR: bool = true;
    const SUPPORTS_3D_NR: bool = true;
    const SUPPORTS_PICTURE_EFFECT: bool = true;
}

impl Presets for SonyFR7 {
    const MAX_PRESETS: u8 = 255;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..25;
    const SUPPORTS_PRESET_TOUR: bool = true;
    const SUPPORTS_PRESET_THUMBNAIL: bool = true;
}

impl Power for SonyFR7 {
    const POWER_ON_TIME: Duration = Duration::from_secs(15);
    const SUPPORTS_STANDBY: bool = true;
    const SUPPORTS_WAKE_ON_LAN: bool = true;
}

impl NdFilter for SonyFR7 {
    const ND_MODE: NdFilterMode = NdFilterMode::Variable;
    const ND_STEPS: Option<u8> = None;
}
impl MenuCapability for SonyFR7 {
    const SUPPORTS_DIRECT_CONTROL: bool = true;
}

impl VariableSpeed for SonyFR7 {
    const SUPPORTS_VARIABLE_SPEED: bool = true;
}

impl crate::capabilities::HasAutoExposure for SonyFR7 {}
impl crate::capabilities::HasBacklightCompensation for SonyFR7 {}
impl crate::capabilities::HasWDR for SonyFR7 {}
impl crate::capabilities::HasExposureCompensation for SonyFR7 {}
impl crate::capabilities::HasOnePushWhiteBalance for SonyFR7 {}
impl crate::capabilities::HasColorTemperature for SonyFR7 {}
impl crate::capabilities::HasRGBGain for SonyFR7 {}
impl crate::capabilities::HasAutoFocus for SonyFR7 {}
impl crate::capabilities::HasOnePushFocus for SonyFR7 {}
impl crate::capabilities::HasPushAutoFocus for SonyFR7 {}
impl crate::capabilities::HasHue for SonyFR7 {}
impl crate::capabilities::menu_control::HasDirectMenuControl for SonyFR7 {}
impl crate::capabilities::Tally for SonyFR7 {
    const SUPPORTS_TALLY: bool = true;
}
impl crate::capabilities::HasPictureEffect for SonyFR7 {}

/// Sony BRC-H900 camera profile.
///
/// Professional pan-tilt-zoom camera with advanced features.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonyBRCH900;

impl ProfileMetadata for SonyBRCH900 {
    const MODEL_NAME: &'static str = "Sony BRC-H900";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = SonyEncapsulated;
    const ACK_TIMEOUT: Duration = Duration::from_millis(150);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(6000);
    const DEFAULT_TCP_PORT: u16 = 52381;
    const DEFAULT_UDP_PORT: u16 = 52381;
}

impl PanTilt for SonyBRCH900 {
    const PAN_RANGE: std::ops::Range<i16> = -2700..2701;
    const TILT_RANGE: std::ops::Range<i16> = -300..1201;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 24;
    const PAN_DEGREES_TO_UNITS: f32 = 15.88;
    const TILT_DEGREES_TO_UNITS: f32 = 15.0;
}

impl Zoom for SonyBRCH900 {
    const OPTICAL_ZOOM_MAX: u16 = 0x4000;
    const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 862.3;
}

impl Focus for SonyBRCH900 {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xF000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
}

impl Exposure for SonyBRCH900 {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1F;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = GENERIC_VISCA_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..16;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
}

impl WhiteBalance for SonyBRCH900 {
    const WB_MODES: &'static [WhiteBalanceMode] = SONY_BRC_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
}

impl ImageProcessing for SonyBRCH900 {
    const BRIGHTNESS_RANGE: std::ops::Range<u8> = 0..18;
    const CONTRAST_RANGE: std::ops::Range<u8> = 0..15;
    const SHARPNESS_RANGE: std::ops::Range<u8> = 0..15;
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SUPPORTS_FLIP: bool = true;
    const SUPPORTS_MIRROR: bool = true;
    const SUPPORTS_NOISE_REDUCTION: bool = true;
    const SUPPORTS_2D_NR: bool = true;
    const SUPPORTS_3D_NR: bool = true;
    const SUPPORTS_PICTURE_EFFECT: bool = true;
}

impl Presets for SonyBRCH900 {
    const MAX_PRESETS: u8 = 100;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..25;
    const SUPPORTS_PRESET_TOUR: bool = true;
}

impl Power for SonyBRCH900 {
    const POWER_ON_TIME: Duration = Duration::from_secs(12);
    const SUPPORTS_STANDBY: bool = true;
}
impl MenuCapability for SonyBRCH900 {}
impl crate::capabilities::Tally for SonyBRCH900 {
    const SUPPORTS_TALLY: bool = true;
}
impl crate::capabilities::HasPictureEffect for SonyBRCH900 {}

/// Sony EVI-H100 camera profile.
///
/// Compact HD camera with limited preset support.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonyEVIH100;

impl ProfileMetadata for SonyEVIH100 {
    const MODEL_NAME: &'static str = "Sony EVI-H100";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for SonyEVIH100 {
    const PAN_RANGE: std::ops::Range<i16> = -1440..1441;
    const TILT_RANGE: std::ops::Range<i16> = -480..481;
    const MAX_PAN_SPEED: u8 = 18;
    const MAX_TILT_SPEED: u8 = 18;
    const PAN_DEGREES_TO_UNITS: f32 = 16.0;
    const TILT_DEGREES_TO_UNITS: f32 = 16.0;
}

impl Zoom for SonyEVIH100 {
    const OPTICAL_ZOOM_MAX: u16 = 0x4000;
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 862.3;
}

impl Focus for SonyEVIH100 {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xF000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
}

impl Exposure for SonyEVIH100 {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1C;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = GENERIC_VISCA_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..8;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = false;
}

impl WhiteBalance for SonyEVIH100 {
    const WB_MODES: &'static [WhiteBalanceMode] = SONY_BRC_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
}

impl Presets for SonyEVIH100 {
    const MAX_PRESETS: u8 = 6;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..20;
    const SUPPORTS_PRESET_TOUR: bool = false;
}

impl Power for SonyEVIH100 {
    const POWER_ON_TIME: Duration = Duration::from_secs(8);
    const SUPPORTS_STANDBY: bool = true;
}

impl ImageProcessing for SonyEVIH100 {
    const BRIGHTNESS_RANGE: std::ops::Range<u8> = 0..0;
    const CONTRAST_RANGE: std::ops::Range<u8> = 0..0;
    const SHARPNESS_RANGE: std::ops::Range<u8> = 0..0;
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = None;
    const SUPPORTS_FLIP: bool = true;
    const SUPPORTS_MIRROR: bool = true;
    const SUPPORTS_NOISE_REDUCTION: bool = true;
    const SUPPORTS_2D_NR: bool = false;
    const SUPPORTS_3D_NR: bool = false;
}

impl MenuCapability for SonyEVIH100 {}
impl crate::capabilities::Tally for SonyEVIH100 {}

/// Sony BRC-300 camera profile.
///
/// Legacy camera with unsigned coordinate system.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonyBRC300;

impl ProfileMetadata for SonyBRC300 {
    const MODEL_NAME: &'static str = "Sony BRC-300";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for SonyBRC300 {
    const PAN_RANGE: std::ops::Range<i16> = -1170..1171;
    const TILT_RANGE: std::ops::Range<i16> = -390..391;
    const MAX_PAN_SPEED: u8 = 18;
    const MAX_TILT_SPEED: u8 = 17;
    const PAN_DEGREES_TO_UNITS: f32 = 13.0;
    const TILT_DEGREES_TO_UNITS: f32 = 13.0;
    const COORDINATE_SYSTEM: CoordinateSystem = CoordinateSystem::UnsignedCentered;
}

impl Zoom for SonyBRC300 {
    const OPTICAL_ZOOM_MAX: u16 = 0x1068;
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 455.1;
}

impl Focus for SonyBRC300 {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xC000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
}

impl Exposure for SonyBRC300 {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x11;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = GENERIC_VISCA_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..7;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = false;
}

impl WhiteBalance for SonyBRC300 {
    const WB_MODES: &'static [WhiteBalanceMode] = GENERIC_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = false;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = None;
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = None;
}

impl Presets for SonyBRC300 {
    const MAX_PRESETS: u8 = 16;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..18;
    const SUPPORTS_PRESET_TOUR: bool = false;
}

impl Power for SonyBRC300 {
    const POWER_ON_TIME: Duration = Duration::from_secs(10);
    const SUPPORTS_STANDBY: bool = false;
}

impl ImageProcessing for SonyBRC300 {
    const BRIGHTNESS_RANGE: std::ops::Range<u8> = 0..0;
    const CONTRAST_RANGE: std::ops::Range<u8> = 0..0;
    const SHARPNESS_RANGE: std::ops::Range<u8> = 0..0;
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = None;
    const SUPPORTS_FLIP: bool = false;
    const SUPPORTS_MIRROR: bool = false;
    const SUPPORTS_NOISE_REDUCTION: bool = false;
    const SUPPORTS_2D_NR: bool = false;
    const SUPPORTS_3D_NR: bool = false;
}

impl MenuCapability for SonyBRC300 {}
impl crate::capabilities::Tally for SonyBRC300 {}

/// Nearus BRC-300 camera profile.
///
/// Rebranded Sony BRC-300 with slight variations.
#[derive(Debug, Default, Clone, Copy)]
pub struct NearusBRC300;

impl ProfileMetadata for NearusBRC300 {
    const MODEL_NAME: &'static str = "Nearus BRC-300";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for NearusBRC300 {
    const PAN_RANGE: std::ops::Range<i16> = -1170..1171;
    const TILT_RANGE: std::ops::Range<i16> = -390..391;
    const MAX_PAN_SPEED: u8 = 18;
    const MAX_TILT_SPEED: u8 = 17;
    const PAN_DEGREES_TO_UNITS: f32 = 13.0;
    const TILT_DEGREES_TO_UNITS: f32 = 13.0;
    const COORDINATE_SYSTEM: CoordinateSystem = CoordinateSystem::UnsignedCentered;
}

impl Zoom for NearusBRC300 {
    const OPTICAL_ZOOM_MAX: u16 = 0x1068;
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 455.1;
}

impl Focus for NearusBRC300 {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xC000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
}

impl Exposure for NearusBRC300 {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x11;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = GENERIC_VISCA_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..7;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = false;
}

impl WhiteBalance for NearusBRC300 {
    const WB_MODES: &'static [WhiteBalanceMode] = GENERIC_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = false;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = None;
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = None;
}

impl Presets for NearusBRC300 {
    const MAX_PRESETS: u8 = 16;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..18;
    const SUPPORTS_PRESET_TOUR: bool = false;
}

impl Power for NearusBRC300 {
    const POWER_ON_TIME: Duration = Duration::from_secs(10);
    const SUPPORTS_STANDBY: bool = false;
}

impl ImageProcessing for NearusBRC300 {
    const BRIGHTNESS_RANGE: std::ops::Range<u8> = 0..16;
    const CONTRAST_RANGE: std::ops::Range<u8> = 0..16;
    const SHARPNESS_RANGE: std::ops::Range<u8> = 0..16;
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = Some(0..16);
    const SUPPORTS_FLIP: bool = false;
    const SUPPORTS_MIRROR: bool = false;
    const SUPPORTS_NOISE_REDUCTION: bool = false;
}

impl MenuCapability for NearusBRC300 {}
impl crate::capabilities::Tally for NearusBRC300 {}

/// PtzOptics G3 camera profile.
///
/// Latest generation PtzOptics camera with enhanced features.
#[derive(Debug, Default, Clone, Copy)]
pub struct PtzOpticsG3;

impl ProfileMetadata for PtzOpticsG3 {
    const MODEL_NAME: &'static str = "PtzOptics G3";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for PtzOpticsG3 {
    const PAN_RANGE: std::ops::Range<i16> = -2448..2449;
    const TILT_RANGE: std::ops::Range<i16> = -432..1297;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 20;
    const PAN_DEGREES_TO_UNITS: f32 = 14.4;
    const TILT_DEGREES_TO_UNITS: f32 = 14.4;
}

impl Zoom for PtzOpticsG3 {
    const OPTICAL_ZOOM_MAX: u16 = 0x4000;
    const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 862.3;
}

impl Focus for PtzOpticsG3 {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xF000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
}

impl Exposure for PtzOpticsG3 {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1D;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = PTZ_OPTICS_G2_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..9;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
}

impl WhiteBalance for PtzOpticsG3 {
    const WB_MODES: &'static [WhiteBalanceMode] = PTZ_OPTICS_G2_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
}

impl ImageProcessing for PtzOpticsG3 {
    const BRIGHTNESS_RANGE: std::ops::Range<u8> = 0..18;
    const CONTRAST_RANGE: std::ops::Range<u8> = 0..15;
    const SHARPNESS_RANGE: std::ops::Range<u8> = 0..15;
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SUPPORTS_FLIP: bool = true;
    const SUPPORTS_MIRROR: bool = true;
    const SUPPORTS_NOISE_REDUCTION: bool = true;
    const SUPPORTS_2D_NR: bool = true;
    const SUPPORTS_3D_NR: bool = true;
}

impl Presets for PtzOpticsG3 {
    const MAX_PRESETS: u8 = 255;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..25;
    const SUPPORTS_PRESET_TOUR: bool = true;
}

impl Power for PtzOpticsG3 {
    const POWER_ON_TIME: Duration = Duration::from_secs(10);
    const SUPPORTS_STANDBY: bool = true;
}
impl MenuCapability for PtzOpticsG3 {}
impl crate::capabilities::Tally for PtzOpticsG3 {}
impl crate::capabilities::HasFocusLock for PtzOpticsG3 {}

impl MotionSync for PtzOpticsG3 {
    const SUPPORTS_MOTION_SYNC: bool = true;
    const MAX_MOTION_SYNC_SPEED: u8 = 24;
}

/// PtzOptics 30X camera profile.
///
/// High-end PtzOptics camera with 30x optical zoom.
#[derive(Debug, Default, Clone, Copy)]
pub struct PtzOptics30X;

impl ProfileMetadata for PtzOptics30X {
    const MODEL_NAME: &'static str = "PtzOptics 30X";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for PtzOptics30X {
    const PAN_RANGE: std::ops::Range<i16> = -2448..2449;
    const TILT_RANGE: std::ops::Range<i16> = -432..1297;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 20;
    const PAN_DEGREES_TO_UNITS: f32 = 14.4;
    const TILT_DEGREES_TO_UNITS: f32 = 14.4;
}

impl Zoom for PtzOptics30X {
    const OPTICAL_ZOOM_MAX: u16 = 0x7AC0;
    const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7FFF);
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 1043.0;
}

impl Focus for PtzOptics30X {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xF000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
}

impl Exposure for PtzOptics30X {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1D;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = PTZ_OPTICS_G2_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..9;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
}

impl WhiteBalance for PtzOptics30X {
    const WB_MODES: &'static [WhiteBalanceMode] = PTZ_OPTICS_G2_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
}

impl ImageProcessing for PtzOptics30X {
    const BRIGHTNESS_RANGE: std::ops::Range<u8> = 0..18;
    const CONTRAST_RANGE: std::ops::Range<u8> = 0..15;
    const SHARPNESS_RANGE: std::ops::Range<u8> = 0..15;
    const SATURATION_RANGE: Option<std::ops::Range<u8>> = Some(0..15);
    const SUPPORTS_FLIP: bool = true;
    const SUPPORTS_MIRROR: bool = true;
    const SUPPORTS_NOISE_REDUCTION: bool = true;
    const SUPPORTS_2D_NR: bool = true;
    const SUPPORTS_3D_NR: bool = true;
}

impl Presets for PtzOptics30X {
    const MAX_PRESETS: u8 = 100;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..25;
    const SUPPORTS_PRESET_TOUR: bool = false;
}

impl Power for PtzOptics30X {
    const POWER_ON_TIME: Duration = Duration::from_secs(10);
    const SUPPORTS_STANDBY: bool = true;
}
impl MenuCapability for PtzOptics30X {}
impl crate::capabilities::Tally for PtzOptics30X {}

impl MotionSync for PtzOptics30X {
    const SUPPORTS_MOTION_SYNC: bool = true;
    const MAX_MOTION_SYNC_SPEED: u8 = 24;
}

/// Preset ID for PtzOptics G2 cameras (0-89).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct G2PresetId(u8);

impl G2PresetId {
    /// Create a new preset ID with validation.
    pub fn new(id: u8) -> Result<Self, Error> {
        if id <= 89 {
            Ok(Self(id))
        } else {
            Err(Error::InvalidPreset {
                preset: id,
                max: 89,
            })
        }
    }

    /// Home preset (preset 0).
    pub const HOME: Self = Self(0);

    /// First user preset.
    pub const PRESET1: Self = Self(1);
}

impl fmt::Display for G2PresetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            write!(f, "Home")
        } else {
            write!(f, "Preset {}", self.0)
        }
    }
}

impl From<G2PresetId> for u8 {
    fn from(preset: G2PresetId) -> Self {
        preset.0
    }
}

impl TryFrom<u8> for G2PresetId {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Gain values for PtzOptics G2 cameras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::ViscaEnum)]
pub enum G2Gain {
    /// 0dB gain
    Gain0dB = 0,
    /// 3dB gain
    Gain3dB = 1,
    /// 6dB gain
    Gain6dB = 2,
    /// 9dB gain
    Gain9dB = 3,
    /// 12dB gain
    Gain12dB = 4,
    /// 15dB gain
    Gain15dB = 5,
    /// 18dB gain
    Gain18dB = 6,
    /// 21dB gain
    Gain21dB = 7,
    /// 24dB gain
    Gain24dB = 8,
}

/// Profile group for runtime polymorphism and profile dispatch.
///
/// Groups camera profiles by their protocol compatibility and behavioral characteristics.
/// This enables runtime profile selection without requiring separate implementations
/// for each profile variant.
///
/// # Example
///
/// ```
/// use grafton_visca::camera::profiles::{ProfileId, ProfileGroup};
///
/// let profile = ProfileId::SonyFr7;
/// let group = profile.profile_group();
/// assert_eq!(group, ProfileGroup::SonyProfessional);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum ProfileGroup {
    /// Generic VISCA-compatible cameras with basic features.
    ///
    /// Includes: GenericVisca, Sony BRC-300, Sony EVI-H100, Nearus BRC-300
    ///
    /// Characteristics:
    /// - Raw VISCA protocol (no encapsulation)
    /// - Basic feature set
    /// - Conservative timeouts and capabilities
    GenericVisca,

    /// PtzOptics camera series (G2, G3, 30X).
    ///
    /// Includes: PtzOptics G2, PtzOptics G3, PtzOptics 30X
    ///
    /// Characteristics:
    /// - Raw VISCA protocol
    /// - Extended feature set
    /// - Motion sync support
    /// - Similar pan/tilt ranges and capabilities
    PtzOpticsG2,

    /// Sony professional cameras with advanced features.
    ///
    /// Includes: Sony FR7, Sony BRC-H900
    ///
    /// Characteristics:
    /// - Sony encapsulated protocol
    /// - Professional feature set
    /// - Variable ND filters (FR7)
    /// - Higher preset counts
    SonyProfessional,
}

/// Serializable camera profile identifier.
///
/// This enum provides a serializable way to identify camera profiles for use in
/// configuration files, APIs, and RPC interfaces. Unlike the zero-sized profile
/// marker types (`PtzOpticsG2`, `SonyFR7`, etc.), this enum can be serialized
/// and deserialized with serde.
///
/// # Example
///
/// ```
/// # #[cfg(feature = "serde")] {
/// use grafton_visca::camera::profiles::ProfileId;
///
/// // Serialize to JSON
/// let profile = ProfileId::PtzOpticsG2;
/// let json = serde_json::to_string(&profile).unwrap();
/// assert_eq!(json, "\"ptz-optics-g2\"");
///
/// // Deserialize from JSON
/// let profile: ProfileId = serde_json::from_str("\"sony-fr7\"").unwrap();
/// assert_eq!(profile, ProfileId::SonyFr7);
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum ProfileId {
    /// PtzOptics G2 series cameras
    PtzOpticsG2,
    /// PtzOptics G3 series cameras
    PtzOpticsG3,
    /// PtzOptics 30X cameras
    #[cfg_attr(feature = "serde", serde(rename = "ptzoptics-30x"))]
    PtzOptics30X,
    /// Sony FR7 camera
    SonyFr7,
    /// Sony BRC-H900 camera
    #[cfg_attr(feature = "serde", serde(rename = "sony-brch900"))]
    SonyBrcH900,
    /// Sony EVI-H100 camera
    #[cfg_attr(feature = "serde", serde(rename = "sony-evih100"))]
    SonyEviH100,
    /// Sony BRC-300 camera
    #[cfg_attr(feature = "serde", serde(rename = "sony-brc300"))]
    SonyBrc300,
    /// Nearus BRC-300 camera
    #[cfg_attr(feature = "serde", serde(rename = "nearus-brc300"))]
    NearusBrc300,
    /// Generic VISCA-compatible camera
    #[default]
    GenericVisca,
}

impl ProfileId {
    /// Returns the human-readable display name for this profile.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// let profile = ProfileId::PtzOpticsG2;
    /// assert_eq!(profile.display_name(), "PtzOptics G2");
    /// ```
    pub const fn display_name(&self) -> &'static str {
        match self {
            ProfileId::PtzOpticsG2 => PtzOpticsG2::MODEL_NAME,
            ProfileId::PtzOpticsG3 => PtzOpticsG3::MODEL_NAME,
            ProfileId::PtzOptics30X => PtzOptics30X::MODEL_NAME,
            ProfileId::SonyFr7 => SonyFR7::MODEL_NAME,
            ProfileId::SonyBrcH900 => SonyBRCH900::MODEL_NAME,
            ProfileId::SonyEviH100 => SonyEVIH100::MODEL_NAME,
            ProfileId::SonyBrc300 => SonyBRC300::MODEL_NAME,
            ProfileId::NearusBrc300 => NearusBRC300::MODEL_NAME,
            ProfileId::GenericVisca => GenericVisca::MODEL_NAME,
        }
    }

    /// Returns the default TCP port for this profile.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// let profile = ProfileId::PtzOpticsG2;
    /// assert_eq!(profile.default_tcp_port(), 5678);
    ///
    /// let sony = ProfileId::SonyFr7;
    /// assert_eq!(sony.default_tcp_port(), 52381);
    /// ```
    pub const fn default_tcp_port(&self) -> u16 {
        match self {
            ProfileId::PtzOpticsG2 => PtzOpticsG2::DEFAULT_TCP_PORT,
            ProfileId::PtzOpticsG3 => PtzOpticsG3::DEFAULT_TCP_PORT,
            ProfileId::PtzOptics30X => PtzOptics30X::DEFAULT_TCP_PORT,
            ProfileId::SonyFr7 => SonyFR7::DEFAULT_TCP_PORT,
            ProfileId::SonyBrcH900 => SonyBRCH900::DEFAULT_TCP_PORT,
            ProfileId::SonyEviH100 => SonyEVIH100::DEFAULT_TCP_PORT,
            ProfileId::SonyBrc300 => SonyBRC300::DEFAULT_TCP_PORT,
            ProfileId::NearusBrc300 => NearusBRC300::DEFAULT_TCP_PORT,
            ProfileId::GenericVisca => GenericVisca::DEFAULT_TCP_PORT,
        }
    }

    /// Returns the default UDP port for this profile.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// let profile = ProfileId::PtzOpticsG2;
    /// assert_eq!(profile.default_udp_port(), 1259);
    ///
    /// let sony = ProfileId::SonyFr7;
    /// assert_eq!(sony.default_udp_port(), 52381);
    /// ```
    pub const fn default_udp_port(&self) -> u16 {
        match self {
            ProfileId::PtzOpticsG2 => PtzOpticsG2::DEFAULT_UDP_PORT,
            ProfileId::PtzOpticsG3 => PtzOpticsG3::DEFAULT_UDP_PORT,
            ProfileId::PtzOptics30X => PtzOptics30X::DEFAULT_UDP_PORT,
            ProfileId::SonyFr7 => SonyFR7::DEFAULT_UDP_PORT,
            ProfileId::SonyBrcH900 => SonyBRCH900::DEFAULT_UDP_PORT,
            ProfileId::SonyEviH100 => SonyEVIH100::DEFAULT_UDP_PORT,
            ProfileId::SonyBrc300 => SonyBRC300::DEFAULT_UDP_PORT,
            ProfileId::NearusBrc300 => NearusBRC300::DEFAULT_UDP_PORT,
            ProfileId::GenericVisca => GenericVisca::DEFAULT_UDP_PORT,
        }
    }

    /// Returns the default camera ID for this profile.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// let profile = ProfileId::PtzOpticsG2;
    /// assert_eq!(profile.default_camera_id(), 1);
    /// ```
    pub const fn default_camera_id(&self) -> u8 {
        match self {
            ProfileId::PtzOpticsG2 => PtzOpticsG2::DEFAULT_CAMERA_ID,
            ProfileId::PtzOpticsG3 => PtzOpticsG3::DEFAULT_CAMERA_ID,
            ProfileId::PtzOptics30X => PtzOptics30X::DEFAULT_CAMERA_ID,
            ProfileId::SonyFr7 => SonyFR7::DEFAULT_CAMERA_ID,
            ProfileId::SonyBrcH900 => SonyBRCH900::DEFAULT_CAMERA_ID,
            ProfileId::SonyEviH100 => SonyEVIH100::DEFAULT_CAMERA_ID,
            ProfileId::SonyBrc300 => SonyBRC300::DEFAULT_CAMERA_ID,
            ProfileId::NearusBrc300 => NearusBRC300::DEFAULT_CAMERA_ID,
            ProfileId::GenericVisca => GenericVisca::DEFAULT_CAMERA_ID,
        }
    }

    /// Returns whether this profile uses Sony encapsulation.
    ///
    /// Sony cameras (FR7, BRC-H900) use a special encapsulation format,
    /// while others use raw VISCA.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// assert!(!ProfileId::PtzOpticsG2.uses_sony_encapsulation());
    /// assert!(ProfileId::SonyFr7.uses_sony_encapsulation());
    /// ```
    pub const fn uses_sony_encapsulation(&self) -> bool {
        matches!(self, ProfileId::SonyFr7 | ProfileId::SonyBrcH900)
    }

    /// Returns the profile group for this camera profile.
    ///
    /// Profile groups categorize cameras by their protocol compatibility and
    /// behavioral characteristics. This enables runtime profile selection and
    /// dynamic dispatch without requiring separate implementations for each
    /// profile variant.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::{ProfileId, ProfileGroup};
    ///
    /// // PtzOptics cameras share the same group
    /// assert_eq!(ProfileId::PtzOpticsG2.profile_group(), ProfileGroup::PtzOpticsG2);
    /// assert_eq!(ProfileId::PtzOpticsG3.profile_group(), ProfileGroup::PtzOpticsG2);
    /// assert_eq!(ProfileId::PtzOptics30X.profile_group(), ProfileGroup::PtzOpticsG2);
    ///
    /// // Sony professional cameras share a group
    /// assert_eq!(ProfileId::SonyFr7.profile_group(), ProfileGroup::SonyProfessional);
    /// assert_eq!(ProfileId::SonyBrcH900.profile_group(), ProfileGroup::SonyProfessional);
    ///
    /// // Generic VISCA and simpler Sony cameras
    /// assert_eq!(ProfileId::GenericVisca.profile_group(), ProfileGroup::GenericVisca);
    /// assert_eq!(ProfileId::SonyBrc300.profile_group(), ProfileGroup::GenericVisca);
    /// assert_eq!(ProfileId::SonyEviH100.profile_group(), ProfileGroup::GenericVisca);
    /// assert_eq!(ProfileId::NearusBrc300.profile_group(), ProfileGroup::GenericVisca);
    /// ```
    ///
    /// # Use Cases
    ///
    /// This method is particularly useful for:
    /// - Runtime profile selection in configuration-driven systems
    /// - Dynamic dispatch based on profile characteristics
    /// - Reducing boilerplate in profile-agnostic code
    /// - Enabling trait object creation with appropriate profile types
    pub const fn profile_group(&self) -> ProfileGroup {
        match self {
            ProfileId::PtzOpticsG2 | ProfileId::PtzOpticsG3 | ProfileId::PtzOptics30X => {
                ProfileGroup::PtzOpticsG2
            }
            ProfileId::SonyFr7 | ProfileId::SonyBrcH900 => ProfileGroup::SonyProfessional,
            ProfileId::GenericVisca
            | ProfileId::SonyBrc300
            | ProfileId::SonyEviH100
            | ProfileId::NearusBrc300 => ProfileGroup::GenericVisca,
        }
    }

    /// Returns all available profile IDs.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// let profiles = ProfileId::all();
    /// assert_eq!(profiles.len(), 9);
    /// assert!(profiles.contains(&ProfileId::PtzOpticsG2));
    /// ```
    pub const fn all() -> &'static [ProfileId] {
        &[
            ProfileId::PtzOpticsG2,
            ProfileId::PtzOpticsG3,
            ProfileId::PtzOptics30X,
            ProfileId::SonyFr7,
            ProfileId::SonyBrcH900,
            ProfileId::SonyEviH100,
            ProfileId::SonyBrc300,
            ProfileId::NearusBrc300,
            ProfileId::GenericVisca,
        ]
    }

    /// Returns whether this profile supports TCP transport.
    ///
    /// All camera profiles support TCP connectivity.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// assert!(ProfileId::PtzOpticsG2.supports_tcp());
    /// assert!(ProfileId::SonyFr7.supports_tcp());
    /// ```
    pub const fn supports_tcp(&self) -> bool {
        true // All profiles support TCP
    }

    /// Returns whether this profile supports UDP transport.
    ///
    /// All camera profiles support UDP connectivity.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// assert!(ProfileId::PtzOpticsG2.supports_udp());
    /// assert!(ProfileId::SonyFr7.supports_udp());
    /// ```
    pub const fn supports_udp(&self) -> bool {
        true // All profiles support UDP
    }

    /// Returns whether this profile supports serial (RS-232/RS-422) transport.
    ///
    /// All camera profiles support serial connectivity.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// assert!(ProfileId::PtzOpticsG2.supports_serial());
    /// assert!(ProfileId::SonyFr7.supports_serial());
    /// ```
    pub const fn supports_serial(&self) -> bool {
        true // All profiles support serial
    }

    /// Returns a vendor identifier for this profile.
    ///
    /// Useful for grouping profiles by manufacturer in UIs or documentation.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// assert_eq!(ProfileId::PtzOpticsG2.vendor(), "PtzOptics");
    /// assert_eq!(ProfileId::SonyFr7.vendor(), "Sony");
    /// assert_eq!(ProfileId::GenericVisca.vendor(), "Generic");
    /// ```
    pub const fn vendor(&self) -> &'static str {
        match self {
            ProfileId::PtzOpticsG2 | ProfileId::PtzOpticsG3 | ProfileId::PtzOptics30X => {
                "PtzOptics"
            }
            ProfileId::SonyFr7
            | ProfileId::SonyBrcH900
            | ProfileId::SonyEviH100
            | ProfileId::SonyBrc300 => "Sony",
            ProfileId::NearusBrc300 => "Nearus",
            ProfileId::GenericVisca => "Generic",
        }
    }

    /// Returns a brief description of this profile's capabilities.
    ///
    /// Provides a human-readable summary of the camera's feature set.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileId;
    ///
    /// let desc = ProfileId::SonyFr7.description();
    /// assert!(desc.contains("Professional"));
    /// ```
    pub const fn description(&self) -> &'static str {
        match self {
            ProfileId::PtzOpticsG2 => {
                "20x optical zoom PTZ camera with digital zoom and 90 presets"
            }
            ProfileId::PtzOpticsG3 => {
                "Latest generation PTZ camera with enhanced features and 255 presets"
            }
            ProfileId::PtzOptics30X => {
                "High-end 30x optical zoom PTZ camera with extended zoom range"
            }
            ProfileId::SonyFr7 => {
                "Professional cinema camera with variable ND filter and full feature set"
            }
            ProfileId::SonyBrcH900 => {
                "Professional PTZ camera with advanced image processing and 100 presets"
            }
            ProfileId::SonyEviH100 => "Compact HD PTZ camera with basic feature set",
            ProfileId::SonyBrc300 => "Legacy PTZ camera with unsigned coordinate system",
            ProfileId::NearusBrc300 => "Rebranded Sony BRC-300 with image processing features",
            ProfileId::GenericVisca => "Conservative profile for unknown VISCA-compatible cameras",
        }
    }
}

impl ProfileGroup {
    /// Returns all profile IDs that belong to this group.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::{ProfileId, ProfileGroup};
    ///
    /// let ptzoptics_profiles = ProfileGroup::PtzOpticsG2.profiles();
    /// assert_eq!(ptzoptics_profiles.len(), 3);
    /// assert!(ptzoptics_profiles.contains(&ProfileId::PtzOpticsG2));
    /// assert!(ptzoptics_profiles.contains(&ProfileId::PtzOpticsG3));
    /// assert!(ptzoptics_profiles.contains(&ProfileId::PtzOptics30X));
    /// ```
    pub const fn profiles(&self) -> &'static [ProfileId] {
        match self {
            ProfileGroup::GenericVisca => &[
                ProfileId::GenericVisca,
                ProfileId::SonyBrc300,
                ProfileId::SonyEviH100,
                ProfileId::NearusBrc300,
            ],
            ProfileGroup::PtzOpticsG2 => &[
                ProfileId::PtzOpticsG2,
                ProfileId::PtzOpticsG3,
                ProfileId::PtzOptics30X,
            ],
            ProfileGroup::SonyProfessional => &[ProfileId::SonyFr7, ProfileId::SonyBrcH900],
        }
    }

    /// Returns a human-readable display name for this profile group.
    pub const fn display_name(&self) -> &'static str {
        match self {
            ProfileGroup::GenericVisca => "Generic VISCA",
            ProfileGroup::PtzOpticsG2 => "PtzOptics Series",
            ProfileGroup::SonyProfessional => "Sony Professional",
        }
    }

    /// Returns whether this profile group uses Sony encapsulation protocol.
    ///
    /// Sony Professional cameras use Sony-encapsulated VISCA protocol,
    /// while other groups use raw VISCA protocol.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileGroup;
    ///
    /// assert!(!ProfileGroup::GenericVisca.uses_sony_encapsulation());
    /// assert!(!ProfileGroup::PtzOpticsG2.uses_sony_encapsulation());
    /// assert!(ProfileGroup::SonyProfessional.uses_sony_encapsulation());
    /// ```
    pub const fn uses_sony_encapsulation(&self) -> bool {
        matches!(self, ProfileGroup::SonyProfessional)
    }

    /// Returns whether this profile group supports TCP transport.
    ///
    /// All camera profiles support TCP connectivity.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileGroup;
    ///
    /// assert!(ProfileGroup::GenericVisca.supports_tcp());
    /// assert!(ProfileGroup::PtzOpticsG2.supports_tcp());
    /// assert!(ProfileGroup::SonyProfessional.supports_tcp());
    /// ```
    pub const fn supports_tcp(&self) -> bool {
        true // All profiles support TCP
    }

    /// Returns whether this profile group supports UDP transport.
    ///
    /// All camera profiles support UDP connectivity.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileGroup;
    ///
    /// assert!(ProfileGroup::GenericVisca.supports_udp());
    /// assert!(ProfileGroup::PtzOpticsG2.supports_udp());
    /// assert!(ProfileGroup::SonyProfessional.supports_udp());
    /// ```
    pub const fn supports_udp(&self) -> bool {
        true // All profiles support UDP
    }

    /// Returns whether this profile group supports serial (RS-232/RS-422) transport.
    ///
    /// All camera profiles support serial connectivity.
    ///
    /// # Example
    ///
    /// ```
    /// use grafton_visca::camera::profiles::ProfileGroup;
    ///
    /// assert!(ProfileGroup::GenericVisca.supports_serial());
    /// assert!(ProfileGroup::PtzOpticsG2.supports_serial());
    /// assert!(ProfileGroup::SonyProfessional.supports_serial());
    /// ```
    pub const fn supports_serial(&self) -> bool {
        true // All profiles support serial
    }
}

impl fmt::Display for ProfileGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl fmt::Display for G2Gain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = match self {
            G2Gain::Gain0dB => "0dB",
            G2Gain::Gain3dB => "3dB",
            G2Gain::Gain6dB => "6dB",
            G2Gain::Gain9dB => "9dB",
            G2Gain::Gain12dB => "12dB",
            G2Gain::Gain15dB => "15dB",
            G2Gain::Gain18dB => "18dB",
            G2Gain::Gain21dB => "21dB",
            G2Gain::Gain24dB => "24dB",
        };
        write!(f, "{db}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::nd_filter::NdFilterExt;
    use crate::capabilities::pan_tilt::PanTiltExt;

    #[test]
    fn test_ptzoptics_g2_capabilities() {
        let camera = PtzOpticsG2;

        assert!(camera.validate_pan(0).is_ok());
        assert!(camera.validate_pan(2448).is_ok());
        assert!(camera.validate_pan(2449).is_err());

        assert_eq!(camera.degrees_to_pan_units(170.0), 2448);
        assert_eq!(camera.pan_units_to_degrees(2448), 170.0);
    }

    #[test]
    fn test_nd_filter_capability() {
        let fr7 = SonyFR7;

        assert!(fr7.has_nd_filter());
        assert_eq!(fr7.nd_filter_description(), "Variable ND filter");
        assert!(fr7.validate_nd_filter(128).is_ok());
    }

    #[test]
    fn test_profile_metadata() {
        assert_eq!(PtzOpticsG2::MODEL_NAME, "PtzOptics G2");
        assert_eq!(SonyFR7::MODEL_NAME, "Sony FR7");
    }

    #[test]
    fn test_profile_groups() {
        // Test PtzOptics group
        assert_eq!(
            ProfileId::PtzOpticsG2.profile_group(),
            ProfileGroup::PtzOpticsG2
        );
        assert_eq!(
            ProfileId::PtzOpticsG3.profile_group(),
            ProfileGroup::PtzOpticsG2
        );
        assert_eq!(
            ProfileId::PtzOptics30X.profile_group(),
            ProfileGroup::PtzOpticsG2
        );

        // Test Sony Professional group
        assert_eq!(
            ProfileId::SonyFr7.profile_group(),
            ProfileGroup::SonyProfessional
        );
        assert_eq!(
            ProfileId::SonyBrcH900.profile_group(),
            ProfileGroup::SonyProfessional
        );

        // Test Generic VISCA group
        assert_eq!(
            ProfileId::GenericVisca.profile_group(),
            ProfileGroup::GenericVisca
        );
        assert_eq!(
            ProfileId::SonyBrc300.profile_group(),
            ProfileGroup::GenericVisca
        );
        assert_eq!(
            ProfileId::SonyEviH100.profile_group(),
            ProfileGroup::GenericVisca
        );
        assert_eq!(
            ProfileId::NearusBrc300.profile_group(),
            ProfileGroup::GenericVisca
        );
    }

    #[test]
    fn test_profile_group_profiles() {
        // Test that all profiles are accounted for
        let generic_profiles = ProfileGroup::GenericVisca.profiles();
        assert_eq!(generic_profiles.len(), 4);
        assert!(generic_profiles.contains(&ProfileId::GenericVisca));
        assert!(generic_profiles.contains(&ProfileId::SonyBrc300));
        assert!(generic_profiles.contains(&ProfileId::SonyEviH100));
        assert!(generic_profiles.contains(&ProfileId::NearusBrc300));

        let ptzoptics_profiles = ProfileGroup::PtzOpticsG2.profiles();
        assert_eq!(ptzoptics_profiles.len(), 3);
        assert!(ptzoptics_profiles.contains(&ProfileId::PtzOpticsG2));
        assert!(ptzoptics_profiles.contains(&ProfileId::PtzOpticsG3));
        assert!(ptzoptics_profiles.contains(&ProfileId::PtzOptics30X));

        let sony_pro_profiles = ProfileGroup::SonyProfessional.profiles();
        assert_eq!(sony_pro_profiles.len(), 2);
        assert!(sony_pro_profiles.contains(&ProfileId::SonyFr7));
        assert!(sony_pro_profiles.contains(&ProfileId::SonyBrcH900));

        // Verify bidirectional consistency
        for profile in ProfileId::all() {
            let group = profile.profile_group();
            assert!(
                group.profiles().contains(profile),
                "Profile {:?} not found in its group {:?}",
                profile,
                group
            );
        }
    }

    #[test]
    fn test_profile_group_display() {
        assert_eq!(ProfileGroup::GenericVisca.display_name(), "Generic VISCA");
        assert_eq!(ProfileGroup::PtzOpticsG2.display_name(), "PtzOptics Series");
        assert_eq!(
            ProfileGroup::SonyProfessional.display_name(),
            "Sony Professional"
        );

        assert_eq!(format!("{}", ProfileGroup::GenericVisca), "Generic VISCA");
        assert_eq!(format!("{}", ProfileGroup::PtzOpticsG2), "PtzOptics Series");
    }

    #[test]
    fn test_profile_group_transport_support() {
        // All profile groups support all transports
        for group in &[
            ProfileGroup::GenericVisca,
            ProfileGroup::PtzOpticsG2,
            ProfileGroup::SonyProfessional,
        ] {
            assert!(group.supports_tcp());
            assert!(group.supports_udp());
            assert!(group.supports_serial());
        }
    }

    #[test]
    fn test_profile_group_encapsulation() {
        assert!(!ProfileGroup::GenericVisca.uses_sony_encapsulation());
        assert!(!ProfileGroup::PtzOpticsG2.uses_sony_encapsulation());
        assert!(ProfileGroup::SonyProfessional.uses_sony_encapsulation());
    }

    #[test]
    fn test_profile_transport_support() {
        // All profiles support all transports
        for profile in ProfileId::all() {
            assert!(profile.supports_tcp());
            assert!(profile.supports_udp());
            assert!(profile.supports_serial());
        }
    }

    #[test]
    fn test_profile_vendor() {
        assert_eq!(ProfileId::PtzOpticsG2.vendor(), "PtzOptics");
        assert_eq!(ProfileId::PtzOpticsG3.vendor(), "PtzOptics");
        assert_eq!(ProfileId::PtzOptics30X.vendor(), "PtzOptics");

        assert_eq!(ProfileId::SonyFr7.vendor(), "Sony");
        assert_eq!(ProfileId::SonyBrcH900.vendor(), "Sony");
        assert_eq!(ProfileId::SonyEviH100.vendor(), "Sony");
        assert_eq!(ProfileId::SonyBrc300.vendor(), "Sony");

        assert_eq!(ProfileId::NearusBrc300.vendor(), "Nearus");
        assert_eq!(ProfileId::GenericVisca.vendor(), "Generic");
    }

    #[test]
    fn test_profile_description() {
        let desc = ProfileId::SonyFr7.description();
        assert!(desc.contains("Professional"));
        assert!(desc.contains("ND filter"));

        let desc = ProfileId::PtzOpticsG2.description();
        assert!(desc.contains("20x"));
        assert!(desc.contains("90 presets"));

        let desc = ProfileId::GenericVisca.description();
        assert!(desc.contains("Conservative"));
    }

    #[test]
    fn test_profile_encapsulation_consistency() {
        // Verify profile and group encapsulation match
        for profile in ProfileId::all() {
            let group = profile.profile_group();
            assert_eq!(
                profile.uses_sony_encapsulation(),
                group.uses_sony_encapsulation(),
                "Profile {:?} and group {:?} encapsulation mismatch",
                profile,
                group
            );
        }
    }
}
