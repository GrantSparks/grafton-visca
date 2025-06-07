//! Common imports for grafton-visca users.
//!
//! This module provides a convenient way to import the most commonly used types
//! and traits from grafton-visca.
//!
//! # Example
//! ```
//! use grafton_visca::prelude::*;
//! ```

// Core types
pub use crate::{ViscaDevice, ViscaError, ViscaResponse};

// Client types (feature-gated)
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub use crate::{ViscaClient, ViscaClientPtzExt};

// Parameter types
pub use crate::{
    DynamicRangeLevel, FocusSpeed, PanSpeed, PanTiltDirection, PresetNumber, TiltSpeed, ZoomSpeed,
};

// Extension traits
pub use crate::{
    ViscaExposureExt, ViscaFocusExt, ViscaImageExt, ViscaInquiryExt, ViscaPanTiltExt,
    ViscaPositionExt, ViscaPowerExt, ViscaPresetExt, ViscaTransportExt, ViscaWhiteBalanceExt,
    ViscaZoomExt,
};

// Common commands - only those that are frequently used directly
pub use crate::command::{
    exposure::ExposureMode,
    pan_tilt::PanTiltCommand,
    power::{Power, PowerCommand},
    preset::PresetCommand,
    white_balance::WhiteBalanceMode,
    zoom::ZoomCommand,
};
