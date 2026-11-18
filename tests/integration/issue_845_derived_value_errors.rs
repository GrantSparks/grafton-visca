//! Numeric failures inside library-generated reply decoders are camera faults.
#![allow(clippy::expect_used)]

use grafton_visca::{
    __macro_support::InquiryData,
    error::ErrorKind,
    request::Inquiry,
    types::{GainLevel, GainLimit, SaturationLevel},
    wire::ResponseParser,
    Error, ViscaInquiry,
};

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x4C, response = Gain, typed_response = GainLevel,
    typed_field = gain)]
struct CanonicalGain;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x49, response = Saturation, typed_response = SaturationLevel,
    typed_field = level)]
struct CanonicalSaturation;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x49, response = Saturation)]
struct CanonicalSaturationRaw;

fn custom_gain_parser(payload: &[u8]) -> Result<InquiryData, Error> {
    let &[raw] = payload else {
        return Err(Error::invalid_response_length(1, payload));
    };
    let level = GainLevel::new(GainLimit::new(raw)?.value())?;
    Ok(InquiryData::Gain { gain: level })
}

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x4C, response = Gain, parser = Custom, parse_with = custom_gain_parser)]
struct GeneratedCustomGain;

#[test]
fn canonical_checked_fields_report_camera_range_errors() {
    // Valid nibble framing with a value outside the checked saturation range.
    let payload = [0, 0, 0, 0x0f];
    for error in [
        CanonicalSaturation
            .decoder()
            .decode(&payload)
            .expect_err("invalid camera saturation"),
        CanonicalSaturationRaw
            .decoder()
            .decode(&payload)
            .expect_err("invalid camera saturation"),
    ] {
        assert_eq!(error.kind(), ErrorKind::Protocol, "{error:?}");
        assert!(matches!(error, Error::InvalidResponse { .. }), "{error:?}");
    }
}

#[test]
fn generated_custom_parser_errors_are_protocol_faults_at_the_decode_boundary() {
    for error in [
        GeneratedCustomGain
            .decoder()
            .decode(&[26])
            .expect_err("invalid camera gain"),
        GeneratedCustomGain
            .parse_response(&[26])
            .expect_err("same custom decode boundary"),
    ] {
        assert_eq!(error.kind(), ErrorKind::Protocol, "{error:?}");
        assert!(matches!(error, Error::InvalidResponse { .. }), "{error:?}");
    }
}

#[test]
fn direct_caller_construction_keeps_its_parameter_error() {
    assert_eq!(
        GainLimit::new(26).expect_err("caller value").kind(),
        ErrorKind::InvalidParameter
    );
    // The caller-provided function is distinct from the library's generated
    // decode boundary; only that boundary reclassifies camera input failures.
    assert_eq!(
        custom_gain_parser(&[26])
            .expect_err("direct custom function")
            .kind(),
        ErrorKind::InvalidParameter
    );
    assert_eq!(
        CanonicalGain
            .decoder()
            .decode(&[0, 0, 1, 9])
            .expect("valid gain")
            .value(),
        25
    );
}

#[test]
fn enum_wire_validation_becomes_a_camera_fault_only_at_the_reply_boundary() {
    use grafton_visca::{types::ExposureMode, wire::WireValue};

    let local_error = ExposureMode::from_wire(0xff).expect_err("unknown caller-supplied enum code");
    assert_eq!(local_error.kind(), ErrorKind::InvalidParameter);

    let reply_error = grafton_visca::command::exposure::ModeInquiry
        .decoder()
        .decode(&[0xff])
        .expect_err("unknown enum code in a camera reply");
    assert_eq!(reply_error.kind(), ErrorKind::Protocol);
    assert!(matches!(reply_error, Error::InvalidResponse { actual, .. } if actual == [0xff]));
}

#[test]
fn an_already_checked_response_projects_without_another_constructor() {
    let gain = GainLevel::new(25).unwrap();
    let response = grafton_visca::wire::Response::Inquiry(InquiryData::Gain { gain });
    assert_eq!(CanonicalGain::from_response(response).unwrap(), gain);
}

#[derive(Debug, PartialEq, Eq)]
struct LocalByte(u8);

impl LocalByte {
    fn new(value: u8) -> Result<Self, Error> {
        Ok(Self(value))
    }
}

fn custom_wide_value(payload: &[u8]) -> Result<InquiryData, Error> {
    let &[high, low] = payload else {
        return Err(Error::invalid_response_length(2, payload));
    };
    Ok(InquiryData::ColorTemperature {
        temperature: u16::from_be_bytes([high, low]),
    })
}

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0, response = ColorTemperature, parser = Custom,
    parse_with = custom_wide_value, typed_response = LocalByte,
    typed_field = temperature, typed_constructor = new_u8)]
struct CustomNarrowedValue;

#[test]
fn custom_parser_projections_retain_checked_narrowing() {
    assert_eq!(
        CustomNarrowedValue.decoder().decode(&[0, 5]).unwrap(),
        LocalByte(5)
    );
    let error = CustomNarrowedValue.decoder().decode(&[1, 5]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Protocol);
    assert!(matches!(error, Error::InvalidResponse { .. }));
}
