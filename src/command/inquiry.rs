//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

// Re-export inquiry command structs that are used by the public API
pub use super::inquiry_structs::{
    AutoFocusInquiry, AutoFocusSensitivityInquiry, AutoTraceInquiry,
    AutoWhiteBalanceSensitivityInquiry, BacklightInquiry, BlackWhiteInquiry, BlackWhiteModeInquiry,
    BlueGainInquiry, BlueTuningInquiry, BrightInquiry, BroadcastDomainInquiry,
    ColorTemperatureInquiry, ContrastInquiry, DefogLevelInquiry, DefogModeInquiry, DigitalInquiry,
    DigitalPtzInquiry, DynamicRangeInquiry, ExposureCompensationInquiry,
    ExposureCompensationModeInquiry, ExposureCompensationPositionInquiry, ExposureModeInquiry,
    FlipModeInquiry, FocusModeInquiry, FocusNearLimitInquiry, FocusPositionInquiry,
    FocusRangeInquiry, FocusUnlockInquiry, FocusZoneInquiry, GainInquiry, GainLimitInquiry,
    GammaInquiry, HueInquiry, ImageFlipInquiry, IrisControlInquiry, IrisInquiry, LuminanceInquiry,
    MenuOpenCloseInquiry, MotionSyncModeInquiry, MotionSyncSpeedInquiry, NdFilterInquiry,
    NdFilterPresetInquiry, NightDayModeInquiry, NoiseReduction2DInquiry, NoiseReduction3DInquiry,
    NrLevelInquiry, NrModeInquiry, NrSpeedInquiry, PanTiltPositionInquiry, PictureEffectInquiry,
    PowerInquiry, RedGainInquiry, RedTuningInquiry, ResolutionInquiry, SaturationInquiry,
    SharpnessInquiry, SharpnessModeInquiry, SharpnessPositionInquiry, ShutterInquiry,
    StandbyInquiry, TallyAutoAdjustInquiry, TallyGreenInquiry, TallyStatusInquiry,
    TwoToneModeInquiry, UsbAudioInquiry, VersionInquiry, WhiteBalanceModeInquiry,
    ZoomPositionInquiry,
};
