//! Method implementations for cameras based on their capabilities.
//!
//! Each module contains trait definitions for camera control methods.
//! Methods are implemented directly on the Camera struct with separate async and blocking implementations.
//!
//! Prefer method-call syntax (`camera.power_on()`) or noun accessors
//! (`camera.power().on()`) when calling these traits. The traits use an
//! associated `Mode` future type, so UFCS-style calls such as
//! `PowerControl::power_on(&camera)` can require extra type annotations in
//! generic code.

pub mod color;
pub mod exposure;
pub mod focus;
pub mod image_processing;
pub mod inquiry;
pub mod menu;
pub mod motion;
pub mod motion_sync;
pub mod nd_filter;
pub mod pan_tilt;
pub mod power;
pub mod presets;
pub mod streaming;
pub mod system;
pub mod tally;
pub mod variable_speed;
pub mod white_balance;
pub mod zoom;

pub use self::{
    color::{
        ColorControl, ColorTemperatureControl, OnePushWhiteBalanceControl, RgbGainControl,
        RgbTuningControl,
    },
    exposure::{
        BacklightCompensationControl, BrightnessControl, ExposureCompensationControl,
        ExposureControl, IrisControl, WideDynamicRangeControl,
    },
    focus::{
        AutoFocusSensitivityControl, FocusControl, FocusLockControl, FocusZoneControl,
        OnePushFocusControl, PushAFControl, SnapFocusControl,
    },
    image_processing::{
        ContrastControl, GammaControl, HueControl, ImageFlipControl, ImageFlipModeControl,
        ImageMirrorControl, LuminanceControl, NoiseReduction2DControl, NoiseReduction3DControl,
        PictureEffectControl, SaturationControl, SharpnessControl,
    },
    inquiry::{
        AutoFocusSensitivityInquiryControl, BacklightCompensationInquiryControl,
        BrightnessInquiryControl, ColorTemperatureInquiryControl, ContrastInquiryControl,
        ExposureCompensationInquiryControl, FocusNearLimitInquiryControl, FocusZoneInquiryControl,
        GammaInquiryControl, HueInquiryControl, ImageFlipInquiryControl, InquiryControl,
        IrisInquiryControl, LuminanceInquiryControl, NdFilterInquiryControl,
        NoiseReduction2DInquiryControl, NoiseReduction3DInquiryControl,
        NoiseReductionInquiryControl, PanTiltInquiryControl, PictureEffectInquiryControl,
        RgbGainInquiryControl, RgbTuningInquiryControl, SaturationInquiryControl,
        SharpnessInquiryControl, WideDynamicRangeInquiryControl,
    },
    menu::{DirectMenuControl, MenuControl},
    motion::MotionControl,
    motion_sync::MotionSyncControl,
    nd_filter::NdFilterControl,
    pan_tilt::PanTiltControl,
    power::PowerControl,
    presets::PresetsControl,
    streaming::StreamingControl,
    system::SystemControl,
    tally::TallyControl,
    variable_speed::VariableSpeedControl,
    white_balance::{
        AutoTrackingWhiteBalanceControl, AutoWhiteBalanceSensitivityControl, WhiteBalanceControl,
    },
    zoom::{DigitalZoomControl, DigitalZoomRangeControl, DirectZoomControl, ZoomControl},
};
