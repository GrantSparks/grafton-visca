//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

// Re-export all inquiry command structs from the internal module
pub use super::inquiry_structs::{
    AntiFlickerInquiry, AutoFocusInquiry, AutoFocusSensitivityInquiry, AutoTraceInquiry,
    AutoWhiteBalanceSensitivityInquiry, BacklightInquiry, BlackWhiteInquiry, BlackWhiteModeInquiry,
    BlueGainInquiry, BlueTuningInquiry, BrightInquiry, BroadcastDomainInquiry,
    ColorTemperatureInquiry, ContrastInquiry, DefogLevelInquiry, DefogModeInquiry, DigitalInquiry,
    DigitalPtzInquiry, DynamicRangeInquiry, ExposureCompensationInquiry,
    ExposureCompensationModeInquiry, ExposureCompensationPositionInquiry, ExposureModeInquiry,
    FlipModeInquiry, FocusModeInquiry, FocusNearLimitInquiry, FocusPositionInquiry,
    FocusRangeInquiry, FocusUnlockInquiry, FocusZoneInquiry, GainInquiry, GainLimitInquiry,
    HueInquiry, ImageFlipInquiry, IrisControlInquiry, IrisInquiry, LuminanceInquiry,
    MenuOpenCloseInquiry, MotionSyncModeInquiry, MotionSyncSpeedInquiry, NdFilterInquiry,
    NdFilterPresetInquiry, NightDayModeInquiry, NoiseReduction2DInquiry, NoiseReduction3DInquiry,
    NrLevelInquiry, NrModeInquiry, NrSpeedInquiry, PanTiltPositionInquiry, PictureEffectInquiry,
    PowerInquiry, RedGainInquiry, RedTuningInquiry, ResolutionInquiry, SaturationInquiry,
    SharpnessInquiry, SharpnessModeInquiry, SharpnessPositionInquiry, ShutterInquiry,
    StandbyInquiry, TallyAutoAdjustInquiry, TallyStatusInquiry, TwoToneModeInquiry,
    UsbAudioInquiry, VersionInquiry, WhiteBalanceModeInquiry, ZoomPositionInquiry,
};
