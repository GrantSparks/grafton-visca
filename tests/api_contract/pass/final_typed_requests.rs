use grafton_visca::{
    operation::AppliedOnly, operation::Targeted, request::OperationCommand, request::PlainCommand,
    request::Request, request::SubmissionClass, types::IrisLevel, types::NdFilterDensity,
};

fn plain<C: PlainCommand>() {}

fn inquiry<I: grafton_visca::request::Inquiry>() {}

fn targeted<O: OperationCommand<Targeted>>() {}

fn applied<O: OperationCommand<AppliedOnly>>() {}

fn request<R: Request>() {}

fn main() -> Result<(), grafton_visca::Error> {
    request::<grafton_visca::command::pan_tilt::Home>();
    plain::<grafton_visca::command::pan_tilt::ClearLimit>();
    inquiry::<grafton_visca::command::power::StatusInquiry>();
    targeted::<grafton_visca::command::pan_tilt::Home>();
    applied::<grafton_visca::command::zoom::Stop>();
    targeted::<grafton_visca::command::exposure::IrisReset>();
    targeted::<grafton_visca::command::exposure::IrisUp>();
    targeted::<grafton_visca::command::exposure::IrisDown>();
    targeted::<grafton_visca::command::exposure::SetIris>();
    targeted::<grafton_visca::command::nd_filter::SetDensity>();
    targeted::<grafton_visca::command::nd_filter::DensityUp>();
    targeted::<grafton_visca::command::nd_filter::DensityDown>();
    applied::<grafton_visca::command::focus::PushAutoFocusPress>();
    applied::<grafton_visca::command::focus::PushAutoFocusRelease>();
    request::<grafton_visca::command::exposure::SetIris>();
    request::<grafton_visca::command::nd_filter::SetDensity>();
    let _ = grafton_visca::command::exposure::IrisReset;
    let _ = grafton_visca::command::exposure::IrisUp;
    let _ = grafton_visca::command::exposure::IrisDown;
    let _ = grafton_visca::command::exposure::SetIris::new(IrisLevel::new(1)?);
    let _ = grafton_visca::command::nd_filter::SetDensity::new(NdFilterDensity::new(1)?);
    let _ = grafton_visca::command::nd_filter::DensityUp;
    let _ = grafton_visca::command::nd_filter::DensityDown;
    let _ = grafton_visca::command::focus::PushAutoFocusPress;
    let _ = grafton_visca::command::focus::PushAutoFocusRelease;
    let _ordinary_qos = [
        SubmissionClass::Background,
        SubmissionClass::Normal,
        SubmissionClass::User,
    ];
    Ok(())
}
