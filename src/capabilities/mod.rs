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
pub mod tally;
mod typed_support;
pub(crate) mod typed_support_registry;
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

// Core profile metadata trait
mod profile_metadata;
pub use profile_metadata::{
    HasExposure, HasFocus, HasImageProcessing, HasMenuControl, HasPanTilt, HasPower, HasPresets,
    HasWhiteBalance, HasZoom, InquirySupport, ProfileMetadata, SupportsSerial, SupportsTcp,
    SupportsUdp,
};

pub use typed_support::{ProfileTypedSupport, TypedSupportSet, TypedSupportSurface};

macro_rules! export_typed_support_marker_entry {
    (DirectMenu, $marker:ident) => {
        pub use menu_control::$marker;
    };
    ($surface:ident, $marker:ident) => {
        pub use typed_support::$marker;
    };
}

macro_rules! export_typed_support_markers {
    (
        [
            $(
                {
                    surface: $surface:ident,
                    marker: $marker:ident,
                    bit: $bit:literal,
                    wire: $wire:literal,
                    area: $area:literal,
                    api: $api:literal,
                    surface_doc: $surface_doc:literal,
                    marker_doc: $marker_doc:literal,
                    diagnostic: $diagnostic:literal,
                },
            )*
        ]
    ) => {
        $(export_typed_support_marker_entry!($surface, $marker);)*
    };
}

typed_support_registry::typed_support_registry!(export_typed_support_markers);

// Re-export all capability traits
pub use exposure::Exposure;
pub use focus::Focus;
pub use image_processing::ImageProcessing;
pub use menu_control::MenuCapability;
pub use motion_sync::MotionSyncMetadata;
pub use nd_filter::{NdFilterMetadata, NdFilterMetadataExt, NdFilterMode};
pub use pan_tilt::{PanTilt, PanTiltWireCodec};
pub use power::Power;
pub use presets::Presets;
pub use tally::Tally;
pub use variable_speed::VariableSpeedMetadata;
pub use white_balance::WhiteBalance;
pub use zoom::Zoom;

// Supporting types
mod types;
pub use types::*;

// Validation utilities
mod validation;
pub use validation::ValidationError;

// Structured capabilities response
mod discovery;
pub use discovery::{Capabilities, RuntimeShutterSpeed};

/// Super-trait that encompasses all camera capabilities.
///
/// This trait allows a single generic bound to ensure a type has all the
/// necessary camera capabilities. It combines metadata with all baseline
/// feature traits plus optional-feature metadata for runtime discovery.
///
/// # Example
/// ```ignore
/// fn use_camera<P: Profile>(camera: &Camera<P>) {
///     // All capability traits are available
///     let model = P::MODEL_NAME;
///     let zoom_range = P::ZOOM_SPEED_RANGE;
///     let inquiry = P::INQUIRY_SUPPORT;
///     let has_motion_sync = P::SUPPORTS_MOTION_SYNC;
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
    + Tally
    + MotionSyncMetadata
    + NdFilterMetadata
    + VariableSpeedMetadata
    + ProfileTypedSupport
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
        + Tally
        + MotionSyncMetadata
        + NdFilterMetadata
        + VariableSpeedMetadata
        + ProfileTypedSupport
        + Sized
        + Send
        + Sync
        + 'static
{
}
