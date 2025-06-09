//! VISCA command definitions and traits.
//!
//! This module provides all command types for controlling VISCA cameras,
//! organized by functionality.

// Crate imports
use crate::timeout::CommandCategory;

// For backward compatibility during migration
use crate::error::Error;

// Command modules
pub mod color;
pub mod exposure;
pub mod flip;
pub mod focus;
pub mod gain;
pub mod image;
pub mod inquiry;
pub mod luminance_contrast_sharpness;
pub mod pan_tilt;
pub mod power;
pub mod preset;
pub mod response;
pub mod white_balance;
pub mod zoom;

// Re-export command types
pub use self::{
    color::*,
    exposure::*,
    flip::*,
    focus::*,
    gain::*,
    image::*,
    inquiry::*,
    luminance_contrast_sharpness::*,
    pan_tilt::*,
    power::*,
    preset::*,
    response::{parse_visca_response, Response, ResponseType},
    white_balance::*,
    zoom::*,
};

/// Trait for all VISCA commands.
///
/// This trait must be implemented by all command types to provide:
/// - Serialization to VISCA protocol bytes
/// - Response type information for inquiry commands
/// - Command category for timeout configuration
///
/// # Example Implementation
/// ```no_run
/// # use grafton_visca::command::{Command, ResponseType};
/// # use grafton_visca::timeout::CommandCategory;
/// # use grafton_visca::Error;
/// struct MyCommand;
///
/// impl Command for MyCommand {
///     fn to_bytes(&self) -> Result<Vec<u8>, Error> {
///         // Return VISCA command bytes
///         Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
///     }
///     
///     fn response_type(&self) -> Option<ResponseType> {
///         // Return None for action commands, Some(...) for inquiries
///         None
///     }
///     
///     fn command_category(&self) -> CommandCategory {
///         // Return appropriate category for timeout configuration
///         CommandCategory::Quick
///     }
/// }
/// ```
pub trait Command: Send + Sync {
    /// Converts the command to VISCA protocol bytes.
    ///
    /// The returned bytes should be a complete VISCA command packet,
    /// typically starting with 0x81 and ending with 0xFF.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidParameter` if the command contains invalid parameters
    fn to_bytes(&self) -> Result<Vec<u8>, Error>;

    /// Returns the expected response type for this command.
    ///
    /// - Returns `None` for action commands that only receive ACK/Completion
    /// - Returns `Some(ResponseType::...)` for inquiry commands that receive data
    fn response_type(&self) -> Option<ResponseType>;

    /// Returns the command category for timeout configuration.
    ///
    /// This is used to determine the appropriate timeout duration for the command.
    /// The default implementation returns `CommandCategory::Custom` which uses
    /// the default timeout.
    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }
}

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
    /// Gain inquiry response.
    Gain {
        /// Gain value (0x00=0 to 0x07=7).
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
    WhiteBalance {
        /// Current white balance mode.
        mode: WhiteBalanceMode,
    },
    /// Color temperature inquiry response.
    ColorTemperature {
        /// Color temperature in Kelvin.
        temperature: u16,
    },
    /// Red gain tuning inquiry response.
    RedGain {
        /// Red gain adjustment value (-10 to +10).
        gain: i8,
    },
    /// Blue gain tuning inquiry response.
    BlueGain {
        /// Blue gain adjustment value (-10 to +10).
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
}
