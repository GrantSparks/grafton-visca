//! Method implementations for cameras based on their capabilities.
//!
//! Each module provides blanket trait implementations that add methods
//! to Camera<P, T> when P implements the corresponding capability trait.

pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod presets;
pub mod white_balance;
pub mod zoom;

// Re-export all extension traits
pub use exposure::ExposureMethods;
pub use focus::FocusMethods;
pub use image_processing::ImageProcessingMethods;
pub use nd_filter::NDFilterMethods;
pub use pan_tilt::PanTiltMethods;
pub use power::PowerMethods;
pub use presets::PresetMethods;
pub use white_balance::WhiteBalanceMethods;
pub use zoom::ZoomMethods;
