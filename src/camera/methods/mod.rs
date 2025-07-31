//! Method implementations for cameras based on their capabilities.
//!
//! Each module provides extension traits that add methods to
//! CameraCore, CameraAsync, and CameraBlocking based on the camera's capabilities.

pub mod color;
pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod inquiry;
pub mod menu;
pub mod motion_sync;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod presets;
pub mod streaming;
pub mod system;
pub mod tally;
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

// Re-export extension traits (async)
#[cfg(feature = "async")]
pub use color::ColorOps;
#[cfg(feature = "async")]
pub use exposure::{ExposureCompensationOps, ExposureOps};
#[cfg(feature = "async")]
pub use focus::FocusOps;
#[cfg(feature = "async")]
pub use image_processing::ImageProcessingOps;
#[cfg(feature = "async")]
pub use inquiry::{InquiryOps, PanTiltInquiryOps};
#[cfg(feature = "async")]
pub use menu::MenuControlOps;
pub use menu::MenuControlOpsBlocking;
#[cfg(feature = "async")]
pub use motion_sync::MotionSyncControl;
#[cfg(feature = "async")]
pub use nd_filter::NDFilterOps;
#[cfg(feature = "async")]
pub use pan_tilt::PanTiltOps;
#[cfg(feature = "async")]
pub use power::PowerOps;
#[cfg(feature = "async")]
pub use presets::PresetsOps;
#[cfg(feature = "async")]
pub use streaming::StreamingOps;
#[cfg(feature = "async")]
pub use system::SystemOps;
#[cfg(feature = "async")]
pub use tally::TallyOps;
#[cfg(feature = "async")]
pub use variable_speed::VariableSpeedOps;
pub use variable_speed::VariableSpeedOpsBlocking;
#[cfg(feature = "async")]
pub use white_balance::WhiteBalanceOps;
#[cfg(feature = "async")]
pub use zoom::ZoomOps;

// Re-export blocking traits
pub use color::ColorOpsBlocking;
pub use exposure::{ExposureCompensationOpsBlocking, ExposureOpsBlocking};
pub use focus::FocusOpsBlocking;
pub use image_processing::ImageProcessingOpsBlocking;
pub use inquiry::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking};
pub use motion_sync::MotionSyncControlBlocking;
pub use nd_filter::NDFilterOpsBlocking;
pub use pan_tilt::PanTiltOpsBlocking;
pub use power::PowerOpsBlocking;
pub use presets::PresetsOpsBlocking;
pub use streaming::StreamingOpsBlocking;
pub use system::SystemOpsBlocking;
pub use tally::TallyOpsBlocking;
pub use white_balance::WhiteBalanceOpsBlocking;
pub use zoom::ZoomOpsBlocking;
