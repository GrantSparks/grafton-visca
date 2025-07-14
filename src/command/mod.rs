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
pub mod pan_tilt;
pub mod power;
pub mod preset;
pub mod response;
pub mod system;
pub mod tally;
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
    flip::*,
    focus::*,
    gain::*,
    image::*,
    image_adjustment::*,
    inquiry::*,
    pan_tilt::*,
    power::*,
    preset::*,
    response::{parse_response, Response, ResponseType},
    system::*,
    tally::*,
    white_balance::*,
    zoom::*,
};

// Note: Complex tests moved to tests/ directory for enhanced testing infrastructure

// Command trait has been replaced by EncodeVisca trait

/// Response data from VISCA inquiry commands.
///
/// Each variant represents a different type of inquiry response with its associated data.
/// These are returned wrapped in `Response::InquiryResponse(...)`.
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
    /// Anti-flicker mode inquiry response.
    AntiFlicker {
        /// Current anti-flicker mode (Off, 50Hz, or 60Hz).
        mode: AntiFlickerMode,
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
}
