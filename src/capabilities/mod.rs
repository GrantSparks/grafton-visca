//! Fine-grained capability traits for VISCA camera features.
//!
//! This module defines traits for each camera capability (pan/tilt, zoom, focus, etc.)
//! allowing camera profiles to be composed from only the features they support.
//! This enables compile-time safety where methods for unsupported features don't exist.

#![allow(dead_code)]

pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod presets;
pub mod white_balance;
pub mod zoom;

// Core profile metadata trait
mod profile_metadata;
pub use profile_metadata::{ProfileIntrospection, ProfileMetadata, ProtocolStyle};

// Re-export all capability traits
pub use exposure::Exposure;
pub use focus::Focus;
pub use image_processing::ImageProcessing;
pub use nd_filter::NDFilter;
pub use pan_tilt::PanTilt;
pub use power::Power;
pub use presets::Presets;
pub use white_balance::WhiteBalance;
pub use zoom::Zoom;

// Supporting types
mod types;
pub use types::*;

// Validation utilities
mod validation;
pub use validation::ValidationError;
