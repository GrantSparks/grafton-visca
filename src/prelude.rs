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
#[cfg(feature = "async")]
pub use crate::camera::methods::{
    ColorOps, ExposureOps, FocusOps, ImageProcessingOps, InquiryOps, NDFilterOps,
    PanTiltInquiryOps, PanTiltOps, PowerOps, PresetsOps, SystemOps, TallyOps, WhiteBalanceOps,
    ZoomOps,
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
    AWBSensitivity, ExposureMode, PanTiltDirection, PanTiltLimitCorner, PresetNumber,
    ResolutionMode,
};

// Re-export camera profiles
pub use crate::camera::profiles::{
    GenericVisca, NearusBRC300, PTZOptics30X, PTZOpticsG2, PTZOpticsG3, SonyBRC300, SonyBRCH900,
    SonyEVIH100, SonyFR7,
};

// Re-export the generic camera
pub use crate::camera::generic::{Camera as GenericCamera, UnifiedTransport};

// Ergonomic type aliases for specific camera models
/// PTZOptics G2 camera type alias.
pub type PTZOpticsG2Cam<T> = GenericCamera<PTZOpticsG2, T>;

/// PTZOptics G3 camera type alias.
pub type PTZOpticsG3Cam<T> = GenericCamera<PTZOpticsG3, T>;

/// PTZOptics 30X camera type alias.
pub type PTZOptics30XCam<T> = GenericCamera<PTZOptics30X, T>;

/// Sony FR7 camera type alias.
pub type SonyFR7Cam<T> = GenericCamera<SonyFR7, T>;

/// Sony BRC-H900 camera type alias.
pub type SonyBRCH900Cam<T> = GenericCamera<SonyBRCH900, T>;

/// Sony EVI-H100 camera type alias.
pub type SonyEVIH100Cam<T> = GenericCamera<SonyEVIH100, T>;

/// Sony BRC-300 camera type alias.
pub type SonyBRC300Cam<T> = GenericCamera<SonyBRC300, T>;

/// Nearus BRC-300 camera type alias.
pub type NearusBRC300Cam<T> = GenericCamera<NearusBRC300, T>;

/// Generic VISCA camera type alias.
pub type GenericViscaCam<T> = GenericCamera<GenericVisca, T>;
