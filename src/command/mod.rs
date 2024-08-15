pub mod exposure;
pub mod flip;
pub mod focus;
pub mod image;
pub mod inquiry;
pub mod luminance_contrast_sharpness;
pub mod pan_tilt;
pub mod power;
pub mod preset;
pub mod response;
pub mod white_balance;
pub mod zoom;

pub use exposure::ExposureCommand;
pub use exposure::ExposureMode;
pub use flip::ImageFlipCommand;
pub use focus::FocusCommand;
pub use image::BacklightCommand;
pub use inquiry::InquiryCommand;
pub use luminance_contrast_sharpness::{ContrastCommand, LuminanceCommand, SharpnessCommand};
pub use pan_tilt::PanTiltCommand;
pub use power::PowerCommand;
pub use preset::PresetCommand;
pub use response::{ViscaResponse, ViscaResponseType};
pub use white_balance::WhiteBalanceCommand;
pub use white_balance::WhiteBalanceMode;
pub use zoom::ZoomCommand;

use crate::ViscaError;

pub trait ViscaCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError>;
    fn response_type(&self) -> Option<ViscaResponseType>;
}

// ViscaInquiryResponse defines various response types for inquiry commands.
#[derive(Debug)]
pub enum ViscaInquiryResponse {
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
    // Add other specific inquiry responses as needed.
}
