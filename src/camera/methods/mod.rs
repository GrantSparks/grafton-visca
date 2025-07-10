//! Method implementations for cameras based on their capabilities.
//!
//! Each module provides blanket trait implementations that add methods
//! to Camera<P, T> when P implements the corresponding capability trait.

pub mod color;
pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod inquiry;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod presets;
pub mod system;
pub mod tally;
pub mod white_balance;
pub mod zoom;

// Re-export all extension traits
pub use color::ColorMethodsExt;
pub use exposure::ExposureMethodsExt;
pub use focus::FocusMethodsExt;
pub use image_processing::ImageProcessingMethodsExt;
pub use inquiry::{InquiryMethodsExt, PanTiltInquiryMethodsExt};
pub use nd_filter::NDFilterMethodsExt;
pub use pan_tilt::PanTiltMethodsExt;
pub use power::PowerMethodsExt;
pub use presets::PresetMethodsExt;
pub use system::SystemMethodsExt;
pub use tally::TallyMethodsExt;
pub use white_balance::WhiteBalanceMethodsExt;
pub use zoom::ZoomMethodsExt;
