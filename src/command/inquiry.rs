//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

// Re-export all inquiry command structs from the internal module
pub use super::inquiry_structs::{
    AutoFocusSensitivityInquiry, BlackWhiteInquiry, BlueGainInquiry, BlueTuningInquiry,
    BrightInquiry, ColorTemperatureInquiry, ContrastInquiry, ExposureCompensationInquiry,
    ExposureCompensationModeInquiry, ExposureModeInquiry, FocusNearLimitInquiry,
    FocusPositionInquiry, FocusZoneInquiry, GainInquiry, GainLimitInquiry, GammaInquiry,
    HueInquiry, IrisInquiry, NdFilterInquiry, NoiseReduction2DInquiry, NoiseReduction3DInquiry,
    PanTiltPositionInquiry, PictureEffectInquiry, PowerInquiry, RedGainInquiry, RedTuningInquiry,
    ResolutionInquiry, SaturationInquiry, SharpnessInquiry, SharpnessModeInquiry, ShutterInquiry,
    WhiteBalanceModeInquiry, ZoomPositionInquiry,
};

// The following inquiry structs are available but currently unused:
// AutoFocusInquiry, AutoTraceInquiry, AutoWhiteBalanceSensitivityInquiry,
// BacklightInquiry, BlackWhiteModeInquiry, BroadcastDomainInquiry, DefogLevelInquiry,
// DefogModeInquiry, DigitalInquiry, DigitalPtzInquiry, DynamicRangeInquiry,
// ExposureCompensationPositionInquiry, FlipModeInquiry, FocusModeInquiry,
// FocusRangeInquiry, FocusUnlockInquiry, ImageFlipInquiry, IrisControlInquiry,
// LuminanceInquiry, MenuOpenCloseInquiry, MotionSyncModeInquiry, MotionSyncSpeedInquiry,
// NdFilterPresetInquiry, NightDayModeInquiry, NrLevelInquiry,
// NrModeInquiry, NrSpeedInquiry, SharpnessPositionInquiry, StandbyInquiry,
// TallyAutoAdjustInquiry, TallyStatusInquiry, TwoToneModeInquiry, UsbAudioInquiry
