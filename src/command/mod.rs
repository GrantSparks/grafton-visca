//! VISCA command definitions and traits.
//!
//! This module provides all command types for controlling VISCA cameras,
//! organized by functionality.

// Crate imports removed - no longer needed

// Command modules
pub mod color;
pub mod exposure;
pub mod flip;
pub mod focus;
pub mod gain;
pub mod image;
pub mod image_adjustment;
pub mod inquiry;
mod inquiry_structs; // Internal module for macro-generated inquiry commands
pub mod menu;
pub mod motion_sync;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod preset;
pub mod resolution;
pub mod response;
pub mod streaming;
pub mod system;
pub mod tally;
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

// New const encoding module
pub mod const_encoding;

// New unified EncodeVisca trait
pub mod encode_visca;

// Re-export command types
pub use self::{
    color::*,
    encode_visca::EncodeVisca,
    exposure::*,
    // flip::*,  // Commented out - unused
    focus::*,
    // gain::*,  // Gain is re-exported through exposure module
    image::*,
    image_adjustment::{BlackWhiteMode, NrMode, NrSpeed, SharpnessMode},
    // inquiry::*,  // Individual types are re-exported from inquiry module
    menu::*,
    // motion_sync::*,  // Commands are internal only
    nd_filter::*,
    pan_tilt::*,
    power::*,
    preset::*,
    response::{Response, ResponseType},
    system::{MotionSyncMode, MotionSyncSpeed},
    // tally::*,  // Commented out - unused
    variable_speed::*,
    white_balance::*,
    // zoom::*,  // Commented out - unused
};

// Note: Complex tests moved to tests/ directory for enhanced testing infrastructure

// Command trait has been replaced by EncodeVisca trait

/// Response data from VISCA inquiry commands.
///
/// Each variant represents a different type of inquiry response with its associated data.
/// These are returned wrapped in `Response::Inquiry(...)`.
#[derive(Debug, Copy, Clone)]
pub enum InquiryResponse {
    /// Power status inquiry response.
    Power {
        /// Whether the camera is powered on.
        on: bool,
    },
    /// Pan/Tilt position inquiry response.
    PanTiltPosition {
        /// Current pan position.
        pan: i16,
        /// Current tilt position.
        tilt: i16,
    },

    /// Luminance level inquiry response.
    Luminance(u8),
    /// Contrast level inquiry response.
    Contrast(u8),
    /// Sharpness value inquiry response.
    Sharpness {
        /// Current sharpness value.
        value: u8,
    },
    /// Sharpness mode inquiry response.
    SharpnessMode {
        /// Current sharpness mode.
        mode: SharpnessMode,
    },
    /// Color saturation inquiry response.
    Saturation {
        /// Saturation level (0x0=60% to 0xE=200%).
        level: u8,
    },
    /// Color hue inquiry response.
    Hue {
        /// Hue value (0x0=0 to 0xE=14).
        hue: u8,
    },

    /// Current zoom position inquiry response.
    ZoomPosition {
        /// Zoom position value.
        position: u16,
    },
    /// Current focus position inquiry response.
    FocusPosition {
        /// Focus position value.
        position: u16,
    },
    /// Focus zone inquiry response.
    FocusZone {
        /// Current focus zone setting.
        zone: FocusZone,
    },
    /// Auto-focus sensitivity inquiry response.
    AutoFocusSensitivity {
        /// Current auto-focus sensitivity setting.
        sensitivity: AutoFocusSensitivity,
    },
    /// Focus near limit inquiry response.
    FocusNearLimit {
        /// Near limit position value.
        position: u16,
    },

    /// Exposure mode inquiry response.
    ExposureMode {
        /// Current exposure mode (Auto, Manual, Shutter, Iris, or Bright).
        mode: ExposureMode,
    },
    /// Exposure compensation inquiry response.
    ExposureCompensation {
        /// Exposure compensation value (-7 to +7).
        value: i8,
    },
    /// Exposure compensation mode inquiry response.
    ExposureCompensationMode {
        /// Whether exposure compensation is enabled.
        on: bool,
    },
    /// Gain level inquiry response.
    GainLevel {
        /// Gain level value (0x00=0 to 0x07=7).
        gain: u8,
    },
    /// Gain limit inquiry response.
    GainLimit {
        /// Maximum gain limit (0x0=0 to 0xF=15).
        limit: u8,
    },
    /// Iris position inquiry response.
    Iris {
        /// Iris position (0x0=Close to 0xC=F1.8).
        position: u8,
    },
    /// Shutter speed inquiry response.
    Shutter {
        /// Shutter position (0x01=1/30 to 0x11=1/10000).
        position: u16,
    },
    /// Brightness inquiry response.
    Bright {
        /// Brightness position (0x00=0 to 0x11=17).
        position: u16,
    },
    /// Backlight compensation inquiry response.
    Backlight {
        /// Whether backlight compensation is enabled.
        status: bool,
    },

    /// White balance mode inquiry response.
    WhiteBalanceMode {
        /// Current white balance mode.
        mode: WhiteBalanceMode,
    },
    /// Color temperature inquiry response.
    ColorTemperature {
        /// Color temperature in Kelvin.
        temperature: u16,
    },
    /// Red channel tuning inquiry response.
    RedChannel {
        /// Red channel adjustment value (-10 to +10).
        gain: i8,
    },
    /// Blue channel tuning inquiry response.
    BlueChannel {
        /// Blue channel adjustment value (-10 to +10).
        gain: i8,
    },

    /// Image flip inquiry response.
    ImageFlip {
        /// Whether vertical flip is enabled.
        vertical: bool,
        /// Whether horizontal flip is enabled.
        horizontal: bool,
    },
    /// Black and white mode inquiry response.
    BlackWhite {
        /// Whether black and white mode is enabled.
        on: bool,
    },
    /// 2D noise reduction inquiry response.
    NoiseReduction2D {
        /// 2D noise reduction level.
        level: u8,
    },
    /// 3D noise reduction inquiry response.
    NoiseReduction3D {
        /// 3D noise reduction level.
        level: u8,
    },
    /// Dynamic range control inquiry response.
    DynamicRange {
        /// Dynamic range level (0x0=0 to 0x8=8).
        level: u8,
    },
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
    },
    /// Red tally light state inquiry response.
    TallyRed {
        /// Whether the red tally light is on.
        on: bool,
    },
    /// Green tally light state inquiry response.
    TallyGreen {
        /// Whether the green tally light is on.
        on: bool,
    },
    /// Focus mode inquiry response.
    FocusMode {
        /// Current focus mode (Auto or Manual).
        mode: FocusMode,
    },
    /// Menu open/close status inquiry response.
    MenuOpenClose {
        /// Whether the camera menu is open.
        is_open: bool,
    },
    /// Auto focus enable/disable status inquiry response.
    AutoFocus {
        /// Whether auto focus is enabled.
        enabled: bool,
    },
    /// Tally light status inquiry response.
    TallyStatus {
        /// Whether the red tally light is on.
        red_on: bool,
        /// Whether the green tally light is on.
        green_on: bool,
    },
    /// Video resolution inquiry response.
    /// The value represents the resolution mode (camera-specific encoding).
    Resolution(u8),
    /// Night/Day mode inquiry response.
    NightDayMode {
        /// Whether the camera is in night mode.
        is_night: bool,
    },
    /// ND filter position inquiry response.
    NdFilter {
        /// Current ND filter position (0x00=Clear, 0x01=1/4, 0x02=1/8, etc.).
        position: u8,
    },
    /// Picture effect mode inquiry response.
    PictureEffect {
        /// Current picture effect (0x00=Off, 0x01=Negative, 0x02=B&W, etc.).
        effect: u8,
    },
    /// Flip mode inquiry response.
    FlipMode {
        /// Whether horizontal flip is enabled.
        horizontal: bool,
        /// Whether vertical flip is enabled.
        vertical: bool,
    },
    /// Standby mode inquiry response.
    Standby {
        /// Whether the camera is in standby mode.
        in_standby: bool,
    },
    /// Focus range mode inquiry response.
    FocusRange {
        /// Current focus range setting.
        range: FocusRange,
    },
    /// Iris control mode inquiry response.
    IrisControl {
        /// Whether iris is in auto mode.
        auto: bool,
    },
    /// Defog mode inquiry response.
    DefogMode {
        /// Whether defog is enabled.
        enabled: bool,
    },
    /// Defog level inquiry response.
    DefogLevel {
        /// Current defog strength level (0-8).
        level: u8,
    },
    /// Digital PTZ mode inquiry response.
    DigitalPtz {
        /// Whether digital PTZ is enabled.
        enabled: bool,
    },
    /// Auto white balance sensitivity inquiry response.
    AutoWhiteBalanceSensitivity {
        /// Sensitivity level (Low, Normal, High).
        sensitivity: AutoWhiteBalanceSensitivity,
    },
    /// Exposure compensation position inquiry response.
    ExposureCompensationPosition {
        /// Exposure compensation position value.
        position: u16,
    },
    /// Red channel tuning inquiry response.
    RedTuning {
        /// Red channel tuning level.
        level: u8,
    },
    /// Blue channel tuning inquiry response.
    BlueTuning {
        /// Blue channel tuning level.
        level: u8,
    },
    /// Gamma curve inquiry response.
    Gamma {
        /// Gamma curve setting (0=Standard, 1-4=different gamma curves).
        value: u8,
    },
    /// Auto trace mode inquiry response.
    AutoTrace {
        /// Whether auto trace is enabled.
        enabled: bool,
    },
    /// Focus unlock state inquiry response.
    FocusUnlock {
        /// Whether focus is unlocked.
        unlocked: bool,
    },
    /// Sharpness position inquiry response.
    SharpnessPosition {
        /// Current sharpness position value.
        position: u16,
    },
    /// Noise reduction level inquiry response.
    NrLevel(u8),
    /// Broadcast domain inquiry response.
    BroadcastDomain(u8),
    /// Motion sync mode inquiry response.
    MotionSyncMode {
        /// Current motion sync mode setting.
        mode: MotionSyncMode,
    },
    /// Motion sync speed inquiry response.
    MotionSyncSpeed {
        /// Current motion sync speed setting.
        speed: MotionSyncSpeed,
    },
    /// Noise reduction mode inquiry response.
    NrMode {
        /// Current noise reduction mode setting.
        mode: NrMode,
    },
    /// Noise reduction speed inquiry response.
    NrSpeed {
        /// Current noise reduction speed setting.
        speed: NrSpeed,
    },
    /// Black and white mode inquiry response.
    BlackWhiteMode {
        /// Current black and white mode setting.
        mode: BlackWhiteMode,
    },
    /// USB audio state inquiry response.
    UsbAudio {
        /// Whether USB audio is enabled.
        on: bool,
    },
    /// Two tone mode inquiry response.
    TwoToneMode {
        /// Whether two tone mode is enabled.
        on: bool,
    },
    /// ND filter preset inquiry response.
    NdFilterPreset {
        /// Current ND filter preset number.
        preset: u8,
    },
    /// Digital mode inquiry response.
    Digital {
        /// Whether digital mode is enabled.
        on: bool,
    },
    /// Tally auto adjust inquiry response.
    TallyAutoAdjust {
        /// Whether tally auto adjust is enabled.
        on: bool,
    },
    /// RTMP state inquiry response.
    Rtmp {
        /// Whether RTMP streaming is enabled.
        on: bool,
    },
    /// Zoom out state inquiry response.
    ZoomOut {
        /// Whether zoom out is active.
        active: bool,
    },
    /// Zoom in state inquiry response.
    ZoomIn {
        /// Whether zoom in is active.
        active: bool,
    },
    /// Iris up state inquiry response.
    IrisUp {
        /// Whether iris up is active.
        active: bool,
    },
    /// Iris down state inquiry response.
    IrisDown {
        /// Whether iris down is active.
        active: bool,
    },
    /// Night/day position inquiry response.
    NightDayPosition {
        /// Current night/day position value.
        position: u8,
    },
    /// Focus near/far state inquiry response.
    FocusNearFar {
        /// Whether focus near is active (false = far active).
        near: bool,
    },
    /// Zoom tele/wide state inquiry response.
    ZoomTeleWide {
        /// Whether zoom tele is active (false = wide active).
        tele: bool,
    },
    /// Night/day switch inquiry response.
    NightDaySwitch {
        /// Whether night/day switch is enabled.
        enabled: bool,
    },
}
