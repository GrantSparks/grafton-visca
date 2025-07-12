//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

// Re-export all inquiry command structs from the internal module
pub use super::inquiry_structs::{
    AntiFlickerInquiry, AutoFocusSensitivityInquiry, BacklightInquiry, BlackWhiteInquiry,
    BlueGainInquiry, BrightInquiry, ColorTemperatureInquiry, ContrastInquiry, DynamicRangeInquiry,
    ExposureCompensationInquiry, ExposureCompensationModeInquiry, ExposureModeInquiry,
    FocusNearLimitInquiry, FocusPositionInquiry, FocusZoneInquiry, GainInquiry, GainLimitInquiry,
    HueInquiry, ImageFlipInquiry, IrisInquiry, LuminanceInquiry, NoiseReduction2DInquiry,
    NoiseReduction3DInquiry, PanTiltPositionInquiry, PowerInquiry, RedGainInquiry,
    SaturationInquiry, SharpnessInquiry, SharpnessModeInquiry, ShutterInquiry, VersionInquiry,
    WhiteBalanceModeInquiry, ZoomPositionInquiry,
};
