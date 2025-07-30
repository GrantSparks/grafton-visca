//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

// Re-export inquiry command structs that are used by the public API
pub use super::inquiry_structs::{
    AutoFocusSensitivityInquiry, BlackWhiteInquiry, BlueGainInquiry, BlueTuningInquiry,
    BrightInquiry, ColorTemperatureInquiry, ContrastInquiry, ExposureCompensationInquiry,
    ExposureCompensationModeInquiry, ExposureModeInquiry, FocusNearLimitInquiry,
    FocusPositionInquiry, FocusZoneInquiry, GainInquiry, GainLimitInquiry, GammaInquiry,
    HueInquiry, IrisInquiry, MotionSyncModeInquiry, MotionSyncSpeedInquiry, NdFilterInquiry,
    NoiseReduction2DInquiry, NoiseReduction3DInquiry, PanTiltPositionInquiry, PictureEffectInquiry,
    PowerInquiry, RedGainInquiry, RedTuningInquiry, ResolutionInquiry, SaturationInquiry,
    SharpnessInquiry, SharpnessModeInquiry, ShutterInquiry, WhiteBalanceModeInquiry,
    ZoomPositionInquiry,
};
