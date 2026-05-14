use grafton_visca::{
    AutoFocusSensitivityControl, AutoFocusSensitivityInquiryControl,
    AutoTrackingWhiteBalanceControl, AutoWhiteBalanceSensitivityControl,
    BacklightCompensationControl, BacklightCompensationInquiryControl, ColorControl,
    ColorTemperatureControl, ColorTemperatureInquiryControl, DigitalZoomControl,
    DigitalZoomRangeControl, DirectMenuControl, DirectZoomControl, ExposureCompensationControl,
    ExposureCompensationInquiryControl, ExposureControl, FocusControl, FocusLockControl,
    FocusNearLimitInquiryControl, FocusZoneControl, FocusZoneInquiryControl, GammaControl,
    GammaInquiryControl, HueControl, HueInquiryControl, ImageFlipControl, ImageFlipInquiryControl,
    ImageFlipModeControl, ImageMirrorControl, ImageProcessingControl, InquiryControl, IrisControl,
    IrisInquiryControl, LuminanceControl, LuminanceInquiryControl, MenuControl, MotionControl,
    MotionSyncControl, NdFilterControl, NdFilterInquiryControl, NoiseReduction2DControl,
    NoiseReduction2DInquiryControl, NoiseReduction3DControl, NoiseReduction3DInquiryControl,
    NoiseReductionInquiryControl, OnePushFocusControl, OnePushWhiteBalanceControl, PanTiltControl,
    PanTiltInquiryControl, PictureEffectControl, PictureEffectInquiryControl, PowerControl,
    PresetsControl, PushAFControl, RgbGainControl, RgbGainInquiryControl, RgbTuningControl,
    RgbTuningInquiryControl, SaturationControl, SaturationInquiryControl, SnapFocusControl,
    StreamingControl, SystemControl, TallyControl, VariableSpeedControl, WhiteBalanceControl,
    WideDynamicRangeControl, WideDynamicRangeInquiryControl, ZoomControl,
};

use core::marker::PhantomData;

struct RootControlBounds<T>(PhantomData<T>);

impl<T> RootControlBounds<T> where
    T: PowerControl
        + ZoomControl
        + DirectZoomControl
        + DigitalZoomControl
        + DigitalZoomRangeControl
        + FocusControl
        + OnePushFocusControl
        + SnapFocusControl
        + FocusZoneControl
        + AutoFocusSensitivityControl
        + PanTiltControl
        + PanTiltInquiryControl
        + PresetsControl
        + InquiryControl
        + FocusNearLimitInquiryControl
        + FocusZoneInquiryControl
        + AutoFocusSensitivityInquiryControl
        + IrisInquiryControl
        + ExposureControl
        + IrisControl
        + BacklightCompensationControl
        + WideDynamicRangeControl
        + WhiteBalanceControl
        + OnePushWhiteBalanceControl
        + AutoTrackingWhiteBalanceControl
        + AutoWhiteBalanceSensitivityControl
        + ImageProcessingControl
        + ImageFlipControl
        + ImageMirrorControl
        + ImageFlipModeControl
        + SaturationControl
        + HueControl
        + LuminanceControl
        + GammaControl
        + NoiseReduction2DControl
        + NoiseReduction3DControl
        + PictureEffectControl
        + TallyControl
        + ColorControl
        + ColorTemperatureControl
        + RgbGainControl
        + RgbTuningControl
        + DirectMenuControl
        + ExposureCompensationControl
        + ExposureCompensationInquiryControl
        + BacklightCompensationInquiryControl
        + WideDynamicRangeInquiryControl
        + ColorTemperatureInquiryControl
        + RgbGainInquiryControl
        + RgbTuningInquiryControl
        + SaturationInquiryControl
        + HueInquiryControl
        + LuminanceInquiryControl
        + GammaInquiryControl
        + ImageFlipInquiryControl
        + NoiseReductionInquiryControl
        + NoiseReduction2DInquiryControl
        + NoiseReduction3DInquiryControl
        + PictureEffectInquiryControl
        + FocusLockControl
        + MenuControl
        + MotionControl
        + MotionSyncControl
        + NdFilterControl
        + NdFilterInquiryControl
        + PushAFControl
        + StreamingControl
        + SystemControl
        + VariableSpeedControl
{
}

fn main() {
    let _ = RootControlBounds::<()>(PhantomData);
}
