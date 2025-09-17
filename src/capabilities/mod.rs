//! Fine-grained capability traits for VISCA camera features.
//!
//! This module defines traits for each camera capability (pan/tilt, zoom, focus, etc.)
//! allowing camera profiles to be composed from only the features they support.
//! This enables compile-time safety where methods for unsupported features don't exist.

pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod menu_control;
pub mod motion_sync;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod presets;
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

// Core profile metadata trait
mod profile_metadata;
pub use profile_metadata::{
    HasAutoExposure, HasAutoFocus, HasBacklightCompensation, HasColorTemperature, HasExposure,
    HasExposureCompensation, HasFocus, HasHue, HasImageProcessing, HasLuminance, HasMenuControl,
    HasMotionSync, HasNdFilter, HasOnePushFocus, HasOnePushWhiteBalance, HasPanTilt, HasPower,
    HasPresets, HasRGBGain, HasVariableSpeed, HasWDR, HasWhiteBalance, HasZoom, ProfileMetadata,
    ProtocolStyle,
};

// Re-export all capability traits
pub use exposure::Exposure;
pub use focus::Focus;
pub use image_processing::ImageProcessing;
pub use menu_control::{HasDirectMenuControl, MenuCapability};
pub use motion_sync::MotionSync;
pub use nd_filter::{NdFilter, NdFilterMode};
pub use pan_tilt::PanTilt;
pub use power::Power;
pub use presets::Presets;
pub use variable_speed::VariableSpeed;
pub use white_balance::WhiteBalance;
pub use zoom::Zoom;

// Supporting types
mod types;
pub use types::*;

// Validation utilities
mod validation;
pub use validation::ValidationError;

/// Super-trait that encompasses all camera capabilities.
///
/// This trait allows a single generic bound to ensure a type has all the
/// necessary camera capabilities. It combines metadata with all feature traits.
///
/// # Example
/// ```ignore
/// fn use_camera<P: Profile>(camera: &Camera<P>) {
///     // All capability traits are available
///     let model = P::MODEL_NAME;
///     let zoom_range = P::ZOOM_SPEED_RANGE;
/// }
/// ```
pub trait Profile:
    ProfileMetadata
    + PanTilt
    + Zoom
    + Focus
    + Exposure
    + WhiteBalance
    + ImageProcessing
    + Presets
    + Power
    + MenuCapability
    + Sized
    + Send
    + Sync
    + 'static
{
}

// Blanket implementation for any type that implements all required traits
impl<T> Profile for T where
    T: ProfileMetadata
        + PanTilt
        + Zoom
        + Focus
        + Exposure
        + WhiteBalance
        + ImageProcessing
        + Presets
        + Power
        + MenuCapability
        + Sized
        + Send
        + Sync
        + 'static
{
}
