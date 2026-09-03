use grafton_visca::{completion, raw, Inquiry, OperationCommand, PlainCommand, ResponseDecoder};

fn assert_plain<T: PlainCommand>() {}
fn assert_inquiry<T: Inquiry>() {}
fn assert_targeted<T: OperationCommand<completion::Targeted>>() {}
fn assert_applied<T: OperationCommand<completion::AppliedOnly>>() {}

fn decode(payload: &[u8]) -> grafton_visca::Result<u8> {
    payload
        .first()
        .copied()
        .ok_or_else(|| grafton_visca::Error::InvalidRequest("empty payload".into()))
}

fn main() {
    assert_eq!(raw::MAX_BYTES, 1_024);
    assert_plain::<raw::Plain>();
    assert_inquiry::<raw::Inquiry<u8>>();
    assert_targeted::<raw::Targeted>();
    assert_applied::<raw::AppliedOnly>();

    let plain = raw::Plain::new(
        [0x81, 0x01, 0x02, 0xff],
        grafton_visca::TimeoutClass::Quick,
        grafton_visca::RetryClass::Never,
        grafton_visca::ControlClass::Normal,
    )
    .expect("plain");
    let inquiry = raw::Inquiry::new(
        [0x81, 0x09, 0x04, 0xff],
        grafton_visca::InquiryRoute::try_custom(9).expect("route"),
        ResponseDecoder::from_fn(decode),
        grafton_visca::TimeoutClass::Inquiry,
        grafton_visca::RetryClass::Inquiry,
        grafton_visca::ControlClass::Normal,
    )
    .expect("inquiry");
    let axes = grafton_visca::AffectedAxes::PAN_TILT;
    let targeted = raw::Targeted::new(
        [0x81, 0x01, 0x06, 0xff],
        axes,
        grafton_visca::TimeoutClass::Movement,
        grafton_visca::RetryClass::Movement,
        grafton_visca::ControlClass::User,
    )
    .expect("targeted");
    let applied = raw::AppliedOnly::with_policy(
        [0x81, 0x01, 0x07, 0xff],
        grafton_visca::AffectedAxes::ZOOM,
        // The urgent lane is owner-only; `User` is the ceiling for raw (#679).
        raw::Policy::new(
            grafton_visca::TimeoutClass::Quick,
            grafton_visca::RetryClass::Never,
            grafton_visca::ControlClass::User,
        )
        .expect("policy"),
    )
    .expect("applied-only");

    let _ = (plain, inquiry, targeted, applied);
}
