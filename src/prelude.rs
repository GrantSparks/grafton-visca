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
pub use crate::{
    error::Error, error::Error as ViscaError, // Both for migration
    session::Session, session::Session as ViscaSession, // Both for migration
    ViscaCommand, ViscaDevice, ViscaInquiryResponse, ViscaResponse, ViscaResponseType,
};

// Client types (feature-gated)
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub use crate::{
    unified_client::Client, unified_client::Client as ViscaClient, // Both for migration
    ViscaClientPtzExt,
};

// Parameter types
pub use crate::{
    BrightnessLevel, ContrastLevel, DynamicRangeLevel, FocusSpeed, GainLimit, GainValue,
    IrisLevel, LuminanceLevel, NoiseReduction2DLevel, NoiseReduction3DLevel, PanSpeed,
    PanTiltDirection, PresetNumber, SharpnessLevel, ShutterSpeed, TiltSpeed, ZoomSpeed,
};

// Extension traits
pub use crate::{
    ViscaExposureExt, ViscaFocusExt, ViscaImageExt, ViscaInquiryExt, ViscaPanTiltExt,
    ViscaPositionExt, ViscaPowerExt, ViscaPresetExt, ViscaTransportExt, ViscaWhiteBalanceExt,
    ViscaZoomExt,
};

// Common commands and enums - only those that are frequently used directly
pub use crate::command::{
    exposure::{ExposureCommand, ExposureMode},
    focus::FocusCommand,
    gain::AntiFlickerMode,
    pan_tilt::PanTiltCommand,
    power::{Power, PowerCommand},
    preset::{PresetAction, PresetCommand},
    white_balance::WhiteBalanceMode,
    zoom::ZoomCommand,
};

// Async extensions (feature-gated)
#[cfg(feature = "async-client")]
pub use crate::AsyncViscaExt;
