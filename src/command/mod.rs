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

pub use color::{
    BlueGainCommand, BlueTuningCommand, ColorTemperatureCommand, HueCommand, OnePushTriggerCommand,
    RedGainCommand, RedTuningCommand, SaturationCommand,
};
pub use exposure::{
    BrightCommand, DynamicRangeCommand, ExposureCommand, ExposureCompensationCommand, ExposureMode,
    IrisCommand, ShutterCommand,
};
pub use flip::ImageFlipCommand;
pub use focus::{
    AFSensitivity, AFSensitivityCommand, FocusCommand, FocusNearLimitCommand, FocusZone,
    FocusZoneCommand,
};
pub use gain::{AntiFlickerCommand, AntiFlickerMode, GainCommand, GainLimitCommand};
pub use image::{
    BacklightCommand, BlackWhiteCommand, ImageFlipCombinedCommand, ImageFlipMode,
    NoiseReduction2DCommand, NoiseReduction3DCommand,
};
pub use inquiry::InquiryCommand;
pub use luminance_contrast_sharpness::{
    ContrastCommand, LuminanceCommand, SharpnessCommand, SharpnessMode,
};
pub use pan_tilt::{LimitCorner, PanTiltCommand, PanTiltLimitCommand};
pub use power::PowerCommand;
pub use preset::PresetCommand;
pub use response::{ViscaResponse, ViscaResponseType};
pub use white_balance::WhiteBalanceCommand;
pub use white_balance::WhiteBalanceMode;
pub use zoom::ZoomCommand;

use crate::ViscaError;
use crate::timeout::CommandCategory;

/// Trait for all VISCA commands.
///
/// This trait must be implemented by all command types to provide:
/// - Serialization to VISCA protocol bytes
/// - Response type information for inquiry commands
/// - Command category for timeout configuration
///
/// # Example Implementation
/// ```no_run
/// # use grafton_visca::command::{ViscaCommand, ViscaResponseType};
/// # use grafton_visca::timeout::CommandCategory;
/// # use grafton_visca::ViscaError;
/// struct MyCommand;
///
/// impl ViscaCommand for MyCommand {
///     fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
///         // Return VISCA command bytes
///         Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
///     }
///     
///     fn response_type(&self) -> Option<ViscaResponseType> {
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
pub trait ViscaCommand: Send + Sync {
    /// Converts the command to VISCA protocol bytes.
    ///
    /// The returned bytes should be a complete VISCA command packet,
    /// typically starting with 0x81 and ending with 0xFF.
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError>;

    /// Returns the expected response type for this command.
    ///
    /// - Returns `None` for action commands that only receive ACK/Completion
    /// - Returns `Some(ViscaResponseType::...)` for inquiry commands that receive data
    fn response_type(&self) -> Option<ViscaResponseType>;

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
/// These are returned wrapped in `ViscaResponse::InquiryResponse(...)`.
#[derive(Debug)]
pub enum ViscaInquiryResponse {
    Power { on: bool },
    PanTiltPosition { pan: i16, tilt: i16 },
    Luminance(u8),
    Contrast(u8),
    ZoomPosition { position: u16 },
    FocusPosition { position: u16 },
    Gain { gain: u8 },
    WhiteBalance { mode: WhiteBalanceMode },
    ExposureMode { mode: ExposureMode },
    ExposureCompensation { value: i8 },
    Backlight { status: bool },
    ColorTemperature { temperature: u16 },
    Hue { hue: u8 },
    // New inquiry responses for Sprint 1 features
    Sharpness { value: u8 },
    ExposureCompensationMode { on: bool },
    Iris { position: u8 },
    Shutter { position: u16 },
    Bright { position: u16 },
    GainLimit { limit: u8 },
    AntiFlicker { mode: AntiFlickerMode },
    Saturation { level: u8 },
    RedGain { gain: i8 },
    BlueGain { gain: i8 },
    ImageFlip { vertical: bool, horizontal: bool },
    // Additional inquiry responses for new features
    SharpnessMode { mode: SharpnessMode },
    NoiseReduction2D { level: u8 },
    NoiseReduction3D { level: u8 },
    BlackWhite { on: bool },
    FocusZone { zone: FocusZone },
    AFSensitivity { sensitivity: AFSensitivity },
    FocusNearLimit { position: u16 },
    DynamicRange { level: u8 },
}
