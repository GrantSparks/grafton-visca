//! Public extension regressions for canonical and explicit custom inquiry decoding.

use grafton_visca::{
    __macro_support::InquiryData, request::Inquiry, wire::Response, Error, ViscaInquiry,
};

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x4b, response = Iris)]
struct IrisInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x12, subcode = 0x06, response = PanTiltPosition)]
struct PositionInquiry;

#[derive(Debug, PartialEq, Eq)]
struct LocalLevel(u8);

impl LocalLevel {
    fn new(value: grafton_visca::types::ZoomPosition) -> Result<Self, Error> {
        let value = u8::try_from(value.value())
            .map_err(|_| Error::invalid_response("local level fitting u8", Vec::new()))?;
        Ok(Self(value))
    }
}

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x47, response = ZoomPosition, typed_response = LocalLevel, typed_field = position, typed_constructor = new)]
struct CheckedLocalInquiry;

fn custom(payload: &[u8]) -> Result<InquiryData, Error> {
    Ok(InquiryData::ExposureCompensationMode {
        on: payload == [0xa5],
    })
}

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0, response = Power, parser = Custom, parse_with = custom, data_variant = ExposureCompensationMode, typed_response = bool, typed_field = on)]
struct CustomInquiry;

#[test]
fn canonical_iris_accepts_the_real_four_byte_reply_and_rejects_the_old_selector_width() {
    assert!(matches!(
        IrisInquiry.parse_response(&[0, 0, 1, 4]),
        Ok(InquiryData::Iris { position }) if position.value() == 20
    ));
    assert!(IrisInquiry.parse_response(&[1, 4]).is_err());
    assert!(matches!(
        IrisInquiry.decoder().decode(&[0, 0, 1, 4]),
        Ok(Response::Inquiry(InquiryData::Iris { position })) if position.value() == 20
    ));
}

#[test]
fn derived_positions_preserve_the_profiles_brc300_framing() {
    let profile = grafton_visca::profiles::ProfileSpec::from_compile_time::<
        grafton_visca::profiles::SonyBrc300,
    >()
    .unwrap();
    assert!(matches!(
        PositionInquiry
            .decoder_for_profile(&profile)
            .decode(&[0x0f, 7, 5, 0x0a, 8, 0x0e, 7, 9, 6]),
        Ok(Response::Inquiry(InquiryData::PanTiltPosition {
            pan: -35416,
            tilt: -6250
        }))
    ));
}

#[test]
fn local_typed_paths_resolve_as_written_and_narrowing_never_truncates() {
    assert_eq!(
        CheckedLocalInquiry.decoder().decode(&[0, 0, 0, 5]).unwrap(),
        LocalLevel(5)
    );
    let error = CheckedLocalInquiry
        .decoder()
        .decode(&[0, 1, 0, 5])
        .unwrap_err();
    assert!(matches!(error, Error::InvalidResponse { ref actual, .. } if actual == &[0, 1, 0, 5]));
    assert_eq!(error.kind(), grafton_visca::error::ErrorKind::Protocol);
}

#[test]
fn custom_parser_and_typed_projection_share_the_declared_override() {
    assert!(CustomInquiry.decoder().decode(&[0xa5]).unwrap());
    assert!(!CustomInquiry.decoder().decode(&[0x02]).unwrap());
    assert!(matches!(
        CustomInquiry.parse_response(&[0xa5]),
        Ok(InquiryData::ExposureCompensationMode { on: true })
    ));
}
