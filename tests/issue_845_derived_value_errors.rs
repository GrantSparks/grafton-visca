//! Numeric failures inside library-generated reply decoders are camera faults.
#![allow(clippy::expect_used)]

use grafton_visca::{
    command::{InquiryData, Response, ResponseParser},
    types::GainLevel,
    Error, ErrorKind, Inquiry, ViscaInquiry,
};

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x4C, response = Gain, typed_response = GainLevel,
    typed_field = gain, typed_constructor = new)]
struct CanonicalGain;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x4C, response = Gain, typed_response = GainLevel,
    typed_field = gain, typed_constructor = new_u8)]
struct CanonicalGainViaU8;

fn custom_gain_parser(payload: &[u8]) -> Result<InquiryData, Error> {
    let &[raw] = payload else {
        return Err(Error::invalid_response_length(1, payload));
    };
    let level = GainLevel::new(raw)?;
    Ok(InquiryData::Gain {
        gain: level.value(),
    })
}

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x4C, response = Gain, parser = Custom, parse_with = custom_gain_parser)]
struct GeneratedCustomGain;

#[test]
fn generated_typed_constructor_errors_are_protocol_faults() {
    // Valid nibble framing; 26 exceeds the shared gain maximum of 25.
    let payload = [0, 0, 1, 0x0A];
    for error in [
        CanonicalGain
            .decoder()
            .decode(&payload)
            .expect_err("invalid camera gain"),
        CanonicalGainViaU8
            .decoder()
            .decode(&payload)
            .expect_err("invalid camera gain"),
        CanonicalGain::from_response(Response::Inquiry(InquiryData::Gain { gain: 26 }))
            .expect_err("invalid typed response value"),
        CanonicalGainViaU8::from_response(Response::Inquiry(InquiryData::Gain { gain: 26 }))
            .expect_err("invalid typed response value"),
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
        GainLevel::new(26).expect_err("caller value").kind(),
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
