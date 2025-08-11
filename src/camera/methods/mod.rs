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
#[cfg(feature = "async")]
pub use white_balance::WhiteBalanceOps;
#[cfg(feature = "async")]
pub use zoom::ZoomOps;

// Re-export blocking traits (only when async is disabled)
#[cfg(not(feature = "async"))]
pub use color::ColorOpsBlocking;
#[cfg(not(feature = "async"))]
pub use exposure::{ExposureCompensationOpsBlocking, ExposureOpsBlocking};
#[cfg(not(feature = "async"))]
pub use focus::FocusOpsBlocking;
#[cfg(not(feature = "async"))]
pub use image_processing::ImageProcessingOpsBlocking;
#[cfg(not(feature = "async"))]
pub use inquiry::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking};
#[cfg(not(feature = "async"))]
pub use menu::MenuControlOpsBlocking;
#[cfg(not(feature = "async"))]
pub use motion_sync::MotionSyncControlBlocking;
#[cfg(not(feature = "async"))]
pub use nd_filter::NDFilterOpsBlocking;
#[cfg(not(feature = "async"))]
pub use pan_tilt::PanTiltOpsBlocking;
#[cfg(not(feature = "async"))]
pub use power::PowerOpsBlocking;
#[cfg(not(feature = "async"))]
pub use presets::PresetsOpsBlocking;
#[cfg(not(feature = "async"))]
pub use streaming::StreamingOpsBlocking;
#[cfg(not(feature = "async"))]
pub use system::SystemOpsBlocking;
#[cfg(not(feature = "async"))]
pub use tally::TallyOpsBlocking;
#[cfg(not(feature = "async"))]
pub use variable_speed::VariableSpeedOpsBlocking;
#[cfg(not(feature = "async"))]
pub use white_balance::WhiteBalanceOpsBlocking;
#[cfg(not(feature = "async"))]
pub use zoom::ZoomOpsBlocking;
