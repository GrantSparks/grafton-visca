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
pub use color::ColorOps;
pub use exposure::ExposureOps;
pub use focus::FocusOps;
pub use image_processing::ImageProcessingOps;
pub use inquiry::{InquiryOps, PanTiltInquiryOps};
pub use menu::{MenuControlMethods, MenuControlMethodsBlocking};
pub use motion_sync::MotionSyncControl;
pub use nd_filter::NDFilterOps;
pub use pan_tilt::PanTiltOps;
pub use power::PowerOps;
pub use presets::PresetsOps;
pub use streaming::StreamingMethods;
pub use system::SystemOps;
pub use tally::TallyOps;
pub use variable_speed::{VariableSpeedMethods, VariableSpeedMethodsBlocking};
pub use white_balance::WhiteBalanceOps;
pub use zoom::ZoomOps;

// Re-export blocking traits
pub use color::ColorOpsBlocking;
pub use exposure::ExposureOpsBlocking;
pub use focus::FocusOpsBlocking;
pub use image_processing::ImageProcessingOpsBlocking;
pub use inquiry::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking};
pub use motion_sync::MotionSyncControlBlocking;
pub use nd_filter::NDFilterOpsBlocking;
pub use pan_tilt::PanTiltOpsBlocking;
pub use power::PowerOpsBlocking;
pub use presets::PresetsOpsBlocking;
pub use streaming::StreamingMethodsBlocking;
pub use system::SystemOpsBlocking;
pub use tally::TallyOpsBlocking;
pub use white_balance::WhiteBalanceOpsBlocking;
pub use zoom::ZoomOpsBlocking;
