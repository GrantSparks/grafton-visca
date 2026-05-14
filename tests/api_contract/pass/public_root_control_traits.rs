use grafton_visca::{
    ColorControl, DirectMenuControl, ExposureCompensationControl, ExposureControl, FocusControl,
    FocusLockControl, ImageProcessingControl, InquiryControl, MenuControl, MotionControl,
    MotionSyncControl, NdFilterControl, NdFilterInquiryControl, PanTiltControl,
    PanTiltInquiryControl, PowerControl, PresetsControl, PushAFControl, StreamingControl,
    SystemControl, TallyControl, VariableSpeedControl, WhiteBalanceControl, ZoomControl,
};

use core::marker::PhantomData;

struct RootControlBounds<T>(PhantomData<T>);

impl<T> RootControlBounds<T> where
    T: PowerControl
        + ZoomControl
        + FocusControl
        + PanTiltControl
        + PanTiltInquiryControl
        + PresetsControl
        + InquiryControl
        + ExposureControl
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
