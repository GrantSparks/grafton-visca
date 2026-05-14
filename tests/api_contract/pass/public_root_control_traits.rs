use grafton_visca::{
    AutoFocusSensitivityControl, AutoFocusSensitivityInquiryControl, ColorControl,
    DigitalZoomControl, DigitalZoomRangeControl, DirectMenuControl, DirectZoomControl,
    ExposureCompensationControl, ExposureControl, FocusControl, FocusLockControl,
    FocusNearLimitInquiryControl, FocusZoneControl, FocusZoneInquiryControl,
    ImageProcessingControl, InquiryControl, IrisControl, IrisInquiryControl, MenuControl,
    MotionControl, MotionSyncControl, NdFilterControl, NdFilterInquiryControl, OnePushFocusControl,
    PanTiltControl, PanTiltInquiryControl, PowerControl, PresetsControl, PushAFControl,
    SnapFocusControl, StreamingControl, SystemControl, TallyControl, VariableSpeedControl,
    WhiteBalanceControl, ZoomControl,
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
        + WhiteBalanceControl
        + ImageProcessingControl
        + TallyControl
        + ColorControl
        + DirectMenuControl
        + ExposureCompensationControl
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
