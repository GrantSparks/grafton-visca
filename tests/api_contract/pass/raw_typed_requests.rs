use grafton_visca::{
    operation, raw, request::Inquiry, request::OperationCommand, request::PlainCommand,
    request::ResponseDecoder,
};

fn assert_plain<T: PlainCommand>() {}
fn assert_inquiry<T: Inquiry>() {}
fn assert_targeted<T: OperationCommand<operation::Targeted>>() {}
fn assert_applied<T: OperationCommand<operation::AppliedOnly>>() {}

fn decode(payload: &[u8]) -> grafton_visca::Result<u8> {
    payload
        .first()
        .copied()
        .ok_or_else(|| grafton_visca::Error::InvalidRequest {
            reason: "empty payload".into(),
        })
}

fn main() {
    assert_eq!(raw::MAX_BYTES, 1_024);
    assert_plain::<raw::PlainRequest>();
    assert_inquiry::<raw::InquiryRequest<u8>>();
    assert_targeted::<raw::TargetedRequest>();
    assert_applied::<raw::AppliedOnlyRequest>();

    let plain = raw::PlainRequest::new(
        [0x81, 0x01, 0x02, 0xff],
        grafton_visca::profiles::CommandTimeoutClass::Quick,
        grafton_visca::request::RetryClass::Never,
        grafton_visca::request::SubmissionClass::Normal,
    )
    .expect("plain");
    let inquiry = raw::InquiryRequest::new(
        [0x81, 0x09, 0x04, 0xff],
        ResponseDecoder::from_fn(decode),
        grafton_visca::request::RetryClass::Inquiry,
        grafton_visca::request::SubmissionClass::Normal,
    )
    .expect("inquiry");
    let axes = grafton_visca::types::AffectedAxes::PAN_TILT;
    let targeted = raw::TargetedRequest::new(
        [0x81, 0x01, 0x06, 0xff],
        axes,
        grafton_visca::profiles::CommandTimeoutClass::Movement,
        grafton_visca::request::RetryClass::Movement,
        grafton_visca::request::SubmissionClass::User,
    )
    .expect("targeted");
    let applied = raw::AppliedOnlyRequest::with_policy(
        [0x81, 0x01, 0x07, 0xff],
        grafton_visca::types::AffectedAxes::ZOOM,
        // The urgent lane is owner-only; `User` is the ceiling for raw (#679).
        grafton_visca::raw::OperationPolicy::new(
            grafton_visca::profiles::CommandTimeoutClass::Quick,
            grafton_visca::request::RetryClass::Never,
            grafton_visca::request::SubmissionClass::User,
        ),
    )
    .expect("applied-only");

    let _ = (plain, inquiry, targeted, applied);
}
