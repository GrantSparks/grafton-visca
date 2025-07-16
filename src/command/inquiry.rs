//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

// Re-export all inquiry command structs from the internal module
pub use super::inquiry_structs::{
    AntiFlickerInquiry, AutoFocusSensitivityInquiry,
    BlackWhiteInquiry,
    BlueGainInquiry, 
    ColorTemperatureInquiry, ContrastInquiry,
    ExposureCompensationInquiry,
    ExposureModeInquiry,
    FocusNearLimitInquiry, FocusPositionInquiry,
    FocusZoneInquiry, GainInquiry, GainLimitInquiry,
    HueInquiry, IrisInquiry,
    NoiseReduction2DInquiry, NoiseReduction3DInquiry,
    PanTiltPositionInquiry,
    PowerInquiry, RedGainInquiry, SaturationInquiry,
    SharpnessInquiry, SharpnessModeInquiry, ShutterInquiry,
    WhiteBalanceModeInquiry, ZoomPositionInquiry,
};

// The following inquiry structs are available but currently unused:
// AutoFocusInquiry, AutoTraceInquiry, AutoWhiteBalanceSensitivityInquiry,
// BacklightInquiry, BlackWhiteModeInquiry, BlueTuningInquiry, BrightInquiry,
// BroadcastDomainInquiry, DefogLevelInquiry, DefogModeInquiry, DigitalInquiry,
// DigitalPtzInquiry, DynamicRangeInquiry, ExposureCompensationModeInquiry,
// ExposureCompensationPositionInquiry, FlipModeInquiry, FocusModeInquiry,
// FocusRangeInquiry, FocusUnlockInquiry, ImageFlipInquiry, IrisControlInquiry,
// LuminanceInquiry, MenuOpenCloseInquiry, MotionSyncModeInquiry, MotionSyncSpeedInquiry,
// NdFilterInquiry, NdFilterPresetInquiry, NightDayModeInquiry, NrLevelInquiry,
// NrModeInquiry, NrSpeedInquiry, PictureEffectInquiry, RedTuningInquiry,
// ResolutionInquiry, SharpnessPositionInquiry, StandbyInquiry, TallyAutoAdjustInquiry,
// TallyStatusInquiry, TwoToneModeInquiry, UsbAudioInquiry, VersionInquiry
