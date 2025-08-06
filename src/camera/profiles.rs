//! Camera profile implementations using capability traits.
//!
//! This module contains camera profiles composed from capability traits,
//! enabling compile-time feature detection and type-safe operations.

use std::{borrow::Cow, fmt, time::Duration};

use crate::{
    capabilities::{
        CoordinateSystem, Exposure, Focus, ImageProcessing, MenuControl, MotionSync, NDFilter,
        NDFilterMode, PanTilt, Power, Presets, ProfileMetadata, ProtocolStyle, ShutterSpeed,
        VariableSpeed, WhiteBalance, Zoom,
    },
    error::Error,
    WhiteBalanceMode,
};

mod exposure_constants {
    use crate::capabilities::ShutterSpeed;

    pub const PTZOPTICS_G2_SHUTTER_SPEEDS: &[ShutterSpeed] = &[
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

use exposure_constants::*;

const PTZOPTICS_G2_WB_MODES: &[WhiteBalanceMode] = &[
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

/// PTZOptics G2 camera profile.
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
pub struct PTZOpticsG2;

impl ProfileMetadata for PTZOpticsG2 {
    const MODEL_NAME: &'static str = "PTZOptics G2";
    const DEFAULT_ADDRESS: u8 = 1;
    const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const SUPPORTS_OPERATION_COMPLETE: bool = true; // PTZOptics cameras support 0x51 completion messages
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for PTZOpticsG2 {
    const PAN_RANGE: std::ops::Range<i16> = -2448..2449;
    const TILT_RANGE: std::ops::Range<i16> = -432..1297;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 20;
    const PAN_DEGREES_TO_UNITS: f32 = 14.4; // 2448/170
    const TILT_DEGREES_TO_UNITS: f32 = 14.4; // (1296+432)/120
}

impl Zoom for PTZOpticsG2 {
    const OPTICAL_ZOOM_MAX: u16 = 0x4000; // 20x optical
    const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 862.3; // 0x4000 / (20-1)
}

impl Focus for PTZOpticsG2 {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xF000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
}

impl Exposure for PTZOpticsG2 {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1D;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = PTZOPTICS_G2_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..9;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
}

impl WhiteBalance for PTZOpticsG2 {
    const WB_MODES: &'static [WhiteBalanceMode] = PTZOPTICS_G2_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
}

impl ImageProcessing for PTZOpticsG2 {
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

impl Presets for PTZOpticsG2 {
    const MAX_PRESETS: u8 = 89;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..25;
    const SUPPORTS_PRESET_TOUR: bool = false;
}

impl Power for PTZOpticsG2 {
    const POWER_ON_TIME: Duration = Duration::from_secs(10);
    const SUPPORTS_STANDBY: bool = true;
}

impl MotionSync for PTZOpticsG2 {
    const SUPPORTS_MOTION_SYNC: bool = true;
    const MAX_MOTION_SYNC_SPEED: u8 = 24;
}
impl MenuControl for PTZOpticsG2 {}

impl crate::capabilities::HasAutoExposure for PTZOpticsG2 {}
impl crate::capabilities::HasBacklightCompensation for PTZOpticsG2 {}
impl crate::capabilities::HasWDR for PTZOpticsG2 {}
impl crate::capabilities::HasOnePushWhiteBalance for PTZOpticsG2 {}
impl crate::capabilities::HasAutoFocus for PTZOpticsG2 {}
impl crate::capabilities::HasOnePushFocus for PTZOpticsG2 {}

/// Generic VISCA camera profile.
///
/// Conservative profile for unknown VISCA cameras with basic features only.
#[derive(Debug, Default, Clone, Copy)]
pub struct GenericVisca;

impl ProfileMetadata for GenericVisca {
    const MODEL_NAME: &'static str = "Generic VISCA Camera";
    const DEFAULT_ADDRESS: u8 = 1;
    const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(200);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(10000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for GenericVisca {
    const PAN_RANGE: std::ops::Range<i16> = -2880..2881; // ±180°
    const TILT_RANGE: std::ops::Range<i16> = -1440..1441; // ±90°
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
impl MenuControl for GenericVisca {}

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
    const DEFAULT_ADDRESS: u8 = 1;
    const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::SonyEncapsulated { use_sequence: true };
    const ACK_TIMEOUT: Duration = Duration::from_millis(200);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(8000);
    const BUSY_TIMEOUT: Duration = Duration::from_millis(240);
    const DEFAULT_TCP_PORT: u16 = 52381;
    const DEFAULT_UDP_PORT: u16 = 52381;
}

impl PanTilt for SonyFR7 {
    const PAN_RANGE: std::ops::Range<i16> = -2700..2701; // ±170°
    const TILT_RANGE: std::ops::Range<i16> = -300..1201; // -20° to +80°
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
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = PTZOPTICS_G2_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..16;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
    const SUPPORTS_EXPOSURE_COMP: bool = true;
}

impl WhiteBalance for SonyFR7 {
    const WB_MODES: &'static [WhiteBalanceMode] = PTZOPTICS_G2_WB_MODES;
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

impl NDFilter for SonyFR7 {
    const ND_MODE: NDFilterMode = NDFilterMode::Variable;
    const ND_STEPS: Option<u8> = None;
}
impl MenuControl for SonyFR7 {
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
impl crate::capabilities::HasHue for SonyFR7 {}

/// Sony BRC-H900 camera profile.
///
/// Professional pan-tilt-zoom camera with advanced features.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonyBRCH900;

impl ProfileMetadata for SonyBRCH900 {
    const MODEL_NAME: &'static str = "Sony BRC-H900";
    const DEFAULT_ADDRESS: u8 = 1;
    const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::SonyEncapsulated {
        use_sequence: false,
    };
    const ACK_TIMEOUT: Duration = Duration::from_millis(150);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(6000);
    const DEFAULT_TCP_PORT: u16 = 52381;
    const DEFAULT_UDP_PORT: u16 = 52381;
}

impl PanTilt for SonyBRCH900 {
    const PAN_RANGE: std::ops::Range<i16> = -2700..2701; // ±170°
    const TILT_RANGE: std::ops::Range<i16> = -300..1201; // -20° to +80°
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 24;
    const PAN_DEGREES_TO_UNITS: f32 = 15.88;
    const TILT_DEGREES_TO_UNITS: f32 = 15.0;
}

impl Zoom for SonyBRCH900 {
    const OPTICAL_ZOOM_MAX: u16 = 0x4000; // 20x optical
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
impl MenuControl for SonyBRCH900 {}

/// Sony EVI-H100 camera profile.
///
/// Compact HD camera with limited preset support.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonyEVIH100;

impl ProfileMetadata for SonyEVIH100 {
    const MODEL_NAME: &'static str = "Sony EVI-H100";
    const DEFAULT_ADDRESS: u8 = 1;
    const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for SonyEVIH100 {
    const PAN_RANGE: std::ops::Range<i16> = -1440..1441; // ±90°
    const TILT_RANGE: std::ops::Range<i16> = -480..481; // ±30°
    const MAX_PAN_SPEED: u8 = 18;
    const MAX_TILT_SPEED: u8 = 18;
    const PAN_DEGREES_TO_UNITS: f32 = 16.0;
    const TILT_DEGREES_TO_UNITS: f32 = 16.0;
}

impl Zoom for SonyEVIH100 {
    const OPTICAL_ZOOM_MAX: u16 = 0x4000; // 20x optical
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

impl MenuControl for SonyEVIH100 {}

/// Sony BRC-300 camera profile.
///
/// Legacy camera with unsigned coordinate system.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonyBRC300;

impl ProfileMetadata for SonyBRC300 {
    const MODEL_NAME: &'static str = "Sony BRC-300";
    const DEFAULT_ADDRESS: u8 = 1;
    const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for SonyBRC300 {
    const PAN_RANGE: std::ops::Range<i16> = -1170..1171; // ±90°
    const TILT_RANGE: std::ops::Range<i16> = -390..391; // ±30°
    const MAX_PAN_SPEED: u8 = 18;
    const MAX_TILT_SPEED: u8 = 17;
    const PAN_DEGREES_TO_UNITS: f32 = 13.0;
    const TILT_DEGREES_TO_UNITS: f32 = 13.0;
    const COORDINATE_SYSTEM: CoordinateSystem = CoordinateSystem::UnsignedCentered;
}

impl Zoom for SonyBRC300 {
    const OPTICAL_ZOOM_MAX: u16 = 0x1068; // 10x optical
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 455.1; // 0x1068 / (10-1)
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

impl MenuControl for SonyBRC300 {}

/// Nearus BRC-300 camera profile.
///
/// Rebranded Sony BRC-300 with slight variations.
#[derive(Debug, Default, Clone, Copy)]
pub struct NearusBRC300;

impl ProfileMetadata for NearusBRC300 {
    const MODEL_NAME: &'static str = "Nearus BRC-300";
    const DEFAULT_ADDRESS: u8 = 1;
    const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::RawVisca;
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

impl MenuControl for NearusBRC300 {}

/// PTZOptics G3 camera profile.
///
/// Latest generation PTZOptics camera with enhanced features.
#[derive(Debug, Default, Clone, Copy)]
pub struct PTZOpticsG3;

impl ProfileMetadata for PTZOpticsG3 {
    const MODEL_NAME: &'static str = "PTZOptics G3";
    const DEFAULT_ADDRESS: u8 = 1;
    const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for PTZOpticsG3 {
    const PAN_RANGE: std::ops::Range<i16> = -2448..2449;
    const TILT_RANGE: std::ops::Range<i16> = -432..1297;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 20;
    const PAN_DEGREES_TO_UNITS: f32 = 14.4;
    const TILT_DEGREES_TO_UNITS: f32 = 14.4;
}

impl Zoom for PTZOpticsG3 {
    const OPTICAL_ZOOM_MAX: u16 = 0x4000; // 20x optical
    const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 862.3;
}

impl Focus for PTZOpticsG3 {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xF000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
}

impl Exposure for PTZOpticsG3 {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1D;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = PTZOPTICS_G2_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..9;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
}

impl WhiteBalance for PTZOpticsG3 {
    const WB_MODES: &'static [WhiteBalanceMode] = PTZOPTICS_G2_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
}

impl ImageProcessing for PTZOpticsG3 {
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

impl Presets for PTZOpticsG3 {
    const MAX_PRESETS: u8 = 255;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..25;
    const SUPPORTS_PRESET_TOUR: bool = true;
}

impl Power for PTZOpticsG3 {
    const POWER_ON_TIME: Duration = Duration::from_secs(10);
    const SUPPORTS_STANDBY: bool = true;
}
impl MenuControl for PTZOpticsG3 {}

impl MotionSync for PTZOpticsG3 {
    const SUPPORTS_MOTION_SYNC: bool = true;
    const MAX_MOTION_SYNC_SPEED: u8 = 24;
}

/// PTZOptics 30X camera profile.
///
/// High-end PTZOptics camera with 30x optical zoom.
#[derive(Debug, Default, Clone, Copy)]
pub struct PTZOptics30X;

impl ProfileMetadata for PTZOptics30X {
    const MODEL_NAME: &'static str = "PTZOptics 30X";
    const DEFAULT_ADDRESS: u8 = 1;
    const PROTOCOL_STYLE: ProtocolStyle = ProtocolStyle::RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(100);
    const COMPLETION_TIMEOUT: Duration = Duration::from_millis(5000);
    const DEFAULT_TCP_PORT: u16 = 5678;
    const DEFAULT_UDP_PORT: u16 = 1259;
}

impl PanTilt for PTZOptics30X {
    const PAN_RANGE: std::ops::Range<i16> = -2448..2449;
    const TILT_RANGE: std::ops::Range<i16> = -432..1297;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 20;
    const PAN_DEGREES_TO_UNITS: f32 = 14.4;
    const TILT_DEGREES_TO_UNITS: f32 = 14.4;
}

impl Zoom for PTZOptics30X {
    const OPTICAL_ZOOM_MAX: u16 = 0x7AC0; // 30x optical
    const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7FFF);
    const ZOOM_SPEED_RANGE: std::ops::Range<u8> = 0..8;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 1043.0; // 0x7AC0 / (30-1)
}

impl Focus for PTZOptics30X {
    const FOCUS_NEAR_LIMIT: u16 = 0x1000;
    const FOCUS_FAR_LIMIT: u16 = 0xF000;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
}

impl Exposure for PTZOptics30X {
    const IRIS_RANGE: std::ops::Range<u16> = 0x00..0x1D;
    const SHUTTER_SPEEDS: &'static [ShutterSpeed] = PTZOPTICS_G2_SHUTTER_SPEEDS;
    const GAIN_RANGE: std::ops::Range<u8> = 0..9;
    const SUPPORTS_AUTO_EXPOSURE: bool = true;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
    const SUPPORTS_WDR: bool = true;
}

impl WhiteBalance for PTZOptics30X {
    const WB_MODES: &'static [WhiteBalanceMode] = PTZOPTICS_G2_WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = true;
    const RG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
    const BG_TUNING_RANGE: Option<std::ops::Range<i8>> = Some(-7..8);
}

impl ImageProcessing for PTZOptics30X {
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

impl Presets for PTZOptics30X {
    const MAX_PRESETS: u8 = 100;
    const PRESET_SPEED_RANGE: std::ops::Range<u8> = 1..25;
    const SUPPORTS_PRESET_TOUR: bool = false;
}

impl Power for PTZOptics30X {
    const POWER_ON_TIME: Duration = Duration::from_secs(10);
    const SUPPORTS_STANDBY: bool = true;
}
impl MenuControl for PTZOptics30X {}

impl MotionSync for PTZOptics30X {
    const SUPPORTS_MOTION_SYNC: bool = true;
    const MAX_MOTION_SYNC_SPEED: u8 = 24;
}

/// Preset ID for PTZOptics G2 cameras (0-89).
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

/// Gain values for PTZOptics G2 cameras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl From<G2Gain> for u8 {
    fn from(gain: G2Gain) -> Self {
        gain as u8
    }
}

impl TryFrom<u8> for G2Gain {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(G2Gain::Gain0dB),
            1 => Ok(G2Gain::Gain3dB),
            2 => Ok(G2Gain::Gain6dB),
            3 => Ok(G2Gain::Gain9dB),
            4 => Ok(G2Gain::Gain12dB),
            5 => Ok(G2Gain::Gain15dB),
            6 => Ok(G2Gain::Gain18dB),
            7 => Ok(G2Gain::Gain21dB),
            8 => Ok(G2Gain::Gain24dB),
            _ => Err(Error::InvalidParameter {
                parameter: "gain",
                value: Cow::Owned(value.to_string()),
                reason: Cow::Borrowed("Invalid G2 gain value"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::nd_filter::NDFilterExt;
    use crate::capabilities::pan_tilt::PanTiltExt;

    #[test]
    fn test_ptzoptics_g2_capabilities() {
        let camera = PTZOpticsG2;

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
        assert_eq!(PTZOpticsG2::MODEL_NAME, "PTZOptics G2");
        assert_eq!(PTZOpticsG2::PROTOCOL_STYLE, ProtocolStyle::RawVisca);

        assert_eq!(SonyFR7::MODEL_NAME, "Sony FR7");
        assert!(matches!(
            SonyFR7::PROTOCOL_STYLE,
            ProtocolStyle::SonyEncapsulated { use_sequence: true }
        ));
    }

    #[test]
    fn test_profile_introspection() {
        use crate::capabilities::ProfileIntrospection;

        let g2 = PTZOpticsG2;
        let fr7 = SonyFR7;
        let generic = GenericVisca;

        assert!(!g2.supports_nd_filter());
        assert!(!fr7.supports_nd_filter());
        assert!(!generic.supports_nd_filter());

        let g2_summary = g2.capability_summary();
        assert!(g2_summary.contains("PTZOptics G2"));

        let fr7_summary = fr7.capability_summary();
        assert!(fr7_summary.contains("Sony FR7"));
    }
}
