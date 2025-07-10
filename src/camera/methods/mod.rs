//! Method implementations for cameras based on their capabilities.
//!
//! Each module provides blanket trait implementations that add methods
//! to Camera<P, T> when P implements the corresponding capability trait.

pub mod pan_tilt;
pub mod zoom;
pub mod focus;
pub mod exposure;
pub mod white_balance;
pub mod image_processing;
pub mod presets;
pub mod power;
pub mod nd_filter;

// Re-export all extension traits
pub use pan_tilt::PanTiltMethods;
pub use zoom::ZoomMethods;
pub use focus::FocusMethods;
pub use exposure::ExposureMethods;
pub use white_balance::WhiteBalanceMethods;
pub use image_processing::ImageProcessingMethods;
pub use presets::PresetMethods;
pub use power::PowerMethods;
pub use nd_filter::NDFilterMethods;