//! Prelude module for convenient imports.
//!
//! This module provides a convenient way to import all commonly used traits and types
//! from the grafton-visca library. Instead of importing individual traits, users can
//! simply use the prelude to get all the camera operation traits.
//!
//! # Example
//!
//! ```no_run
//! // Instead of multiple imports:
//! // use grafton_visca::camera::methods::{PowerOps, ZoomOps, PanTiltOps};
//! 
//! // Simply use the prelude:
//! use grafton_visca::prelude::*;
//! use grafton_visca::{Camera, CameraModel};
//! 
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "tokio")]
//! # {
//! use grafton_visca::transport::tokio::Tcp;
//! 
//! let transport = Tcp::connect("192.168.1.100:52381").await?;
//! let camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);
//! 
//! // All trait methods are now available
//! camera.power_on().await?;
//! camera.zoom_stop().await?;
//! camera.pan_tilt_home().await?;
//! # }
//! # Ok(())
//! # }
//! ```

// Re-export all async operation traits
pub use crate::camera::methods::{
    ColorOps, ExposureOps, FocusOps, ImageProcessingOps, InquiryOps, NDFilterOps,
    PanTiltInquiryOps, PanTiltOps, PowerOps, PresetsOps, SystemOps, TallyOps, 
    WhiteBalanceOps, ZoomOps,
};

// Re-export all blocking operation traits
pub use crate::camera::methods::{
    ColorOpsBlocking, ExposureOpsBlocking, FocusOpsBlocking, ImageProcessingOpsBlocking,
    InquiryOpsBlocking, NDFilterOpsBlocking, PanTiltInquiryOpsBlocking, PanTiltOpsBlocking,
    PowerOpsBlocking, PresetsOpsBlocking, SystemOpsBlocking, TallyOpsBlocking,
    WhiteBalanceOpsBlocking, ZoomOpsBlocking,
};

// Re-export commonly used types
pub use crate::{Camera, CameraModel, Error};

// Re-export command enums that users need
pub use crate::{
    ExposureMode, PanTiltDirection, PresetNumber,
};