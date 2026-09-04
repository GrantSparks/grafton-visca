use grafton_visca::{
    command::{NdFilterValue, PowerInquiry},
    completion::{AppliedOnly, Targeted},
    request::builtin::{
        IrisDirect, IrisDown, IrisReset, IrisUp, NdFilterDirect, NdFilterStepDown, NdFilterStepUp,
        PanTiltHome, PanTiltLimitClear, PushAfPress, PushAfRelease, ZoomStop,
    },
    types::IrisLevel,
    OperationCommand, PlainCommand, Request, SubmissionClass,
};

fn plain<C: PlainCommand>() {}

fn inquiry<I: grafton_visca::Inquiry>() {}

fn targeted<O: OperationCommand<Targeted>>() {}

fn applied<O: OperationCommand<AppliedOnly>>() {}

fn request<R: Request>() {}

fn main() -> Result<(), grafton_visca::Error> {
    request::<PanTiltHome>();
    plain::<PanTiltLimitClear>();
    inquiry::<PowerInquiry>();
    targeted::<PanTiltHome>();
    applied::<ZoomStop>();
    targeted::<IrisReset>();
    targeted::<IrisUp>();
    targeted::<IrisDown>();
    targeted::<IrisDirect>();
    targeted::<NdFilterDirect>();
    targeted::<NdFilterStepUp>();
    targeted::<NdFilterStepDown>();
    applied::<PushAfPress>();
    applied::<PushAfRelease>();
    request::<IrisDirect>();
    request::<NdFilterDirect>();
    let _ = IrisReset::new();
    let _ = IrisUp::new();
    let _ = IrisDown::new();
    let _ = IrisDirect::new(IrisLevel::new(1)?);
    let _ = NdFilterDirect::new(NdFilterValue::new(1)?);
    let _ = NdFilterStepUp::new();
    let _ = NdFilterStepDown::new();
    let _ = PushAfPress::new();
    let _ = PushAfRelease::new();
    let _ordinary_qos = [
        SubmissionClass::Background,
        SubmissionClass::Normal,
        SubmissionClass::User,
    ];
    Ok(())
}
