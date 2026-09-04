//! Public regression coverage for the `ViscaInquiry` derive's parser selector.
//!
//! A parser selector enables the inherent convenience method, but it must never
//! create a decoder that differs from `Inquiry::decoder()` for the same
//! `response` kind.

use grafton_visca::{
    command::{InquiryData, Response},
    Error, Inquiry, ViscaInquiry,
};

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x7B, response = Digital, parser = Bool)]
struct BoolInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x2C, response = GainLimit, parser = DirectByte)]
struct DirectByteInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x53, response = NoiseReduction2D, parser = Byte)]
struct ByteInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x47, response = ZoomPosition, parser = Position)]
struct PositionInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x4B,
    response = Iris,
    parser = ExtendedNibble,
    field = position
)]
struct ExtendedNibbleInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x4B, response = Iris, parser = Nibble, field = position)]
struct NibbleInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0xA4, response = FlipState, parser = Flags)]
struct FlagsInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0xA4, response = FlipState, parser = BitFlags)]
struct BitFlagsInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x39,
    response = ExposureMode,
    parser = Mode,
    value_type = ExposureMode
)]
struct ModeInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x39,
    response = ExposureMode,
    parser = ModeEnum,
    value_type = ExposureMode
)]
struct ModeEnumInquiry;

type AliasExposureMode = grafton_visca::command::ExposureMode;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x39,
    response = ExposureMode,
    parser = Mode,
    value_type = crate::AliasExposureMode
)]
struct ModeAliasInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x12,
    subcode = 0x06,
    response = PanTiltPosition,
    parser = PanTilt
)]
struct PanTiltInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x7B,
    response = Digital,
    parser = BoolConvention,
    field = on,
    convention = OnIs03
)]
struct BoolConventionInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x6B,
    response = DigitalPtz,
    parser = BoolConvention,
    field = on,
    convention = OnIs02,
    data_variant = Digital
)]
struct BoolConventionDataVariantInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x4F, response = Hue, parser = LastNibble, field = hue)]
struct LastNibbleInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0xA8, response = TallyStatus, parser = TallyStatus)]
struct TallyStatusInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x05, response = SharpnessMode, parser = SharpnessMode)]
struct SharpnessModeInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x5B, response = Gamma, parser = Gamma)]
struct GammaInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0xA9,
    response = AutoWhiteBalanceSensitivity,
    parser = AutoWbSensitivity
)]
struct AutoWbSensitivityInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x64, response = NdFilter, parser = NdFilter)]
struct NdFilterInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x63, response = PictureEffect, parser = PictureEffect)]
struct PictureEffectInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0xA0, response = DefogLevel, parser = DefogLevel)]
struct DefogLevelInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x2A, response = FocusRange, parser = FocusRange)]
struct FocusRangeInquiry;

#[allow(dead_code)]
fn custom_parser(payload: &[u8]) -> Result<InquiryData, Error> {
    Ok(InquiryData::Power {
        on: payload == [0xA5],
    })
}

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x00,
    response = Power,
    parser = Custom,
    parse_with = custom_parser
)]
struct CustomInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x00,
    response = Power,
    parser = Custom,
    parse_with = custom_parser,
    typed_response = bool,
    typed_field = on
)]
struct CustomTypedInquiry;

fn decoded_inquiry<I>(inquiry: &I, payload: &[u8]) -> Result<InquiryData, Error>
where
    I: Inquiry<Response = Response>,
{
    inquiry
        .decoder()
        .decode(payload)
        .and_then(|response| match response {
            Response::Inquiry(data) => Ok(data),
            _ => Err(Error::UnexpectedResponseType),
        })
}

macro_rules! assert_canonical_decode {
    ($inquiry:expr, $payload:expr) => {{
        let inquiry = $inquiry;
        let parsed = inquiry.parse_response($payload);
        let decoded = decoded_inquiry(&inquiry, $payload);
        assert_eq!(
            format!("{parsed:?}"),
            format!("{decoded:?}"),
            "{} diverged for payload {:02X?}",
            stringify!($inquiry),
            $payload,
        );
    }};
}

#[test]
fn every_parser_selector_uses_the_response_kind_decoder() {
    assert_canonical_decode!(BoolInquiry, &[0x03]);
    assert_canonical_decode!(DirectByteInquiry, &[0x07]);
    assert_canonical_decode!(ByteInquiry, &[0x07]);
    assert_canonical_decode!(PositionInquiry, &[0x00, 0x01, 0x02, 0x03]);
    assert_canonical_decode!(ExtendedNibbleInquiry, &[0x00, 0x05]);
    assert_canonical_decode!(NibbleInquiry, &[0x00, 0x05]);
    assert_canonical_decode!(FlagsInquiry, &[0x03]);
    assert_canonical_decode!(BitFlagsInquiry, &[0x03]);
    assert_canonical_decode!(ModeInquiry, &[0x00]);
    assert_canonical_decode!(ModeEnumInquiry, &[0x00]);
    assert_canonical_decode!(ModeAliasInquiry, &[0x00]);
    assert_canonical_decode!(PanTiltInquiry, &[0x00; 8]);
    assert_canonical_decode!(BoolConventionInquiry, &[0x03]);
    assert_canonical_decode!(LastNibbleInquiry, &[0x00, 0x00, 0x00, 0x0E]);
    assert_canonical_decode!(TallyStatusInquiry, &[0x03, 0x02]);
    assert_canonical_decode!(SharpnessModeInquiry, &[0x02]);
    assert_canonical_decode!(GammaInquiry, &[0x04]);
    assert_canonical_decode!(AutoWbSensitivityInquiry, &[0x00]);
    assert_canonical_decode!(NdFilterInquiry, &[0x00]);
    assert_canonical_decode!(PictureEffectInquiry, &[0x00]);
    assert_canonical_decode!(DefogLevelInquiry, &[0x05]);
    assert_canonical_decode!(FocusRangeInquiry, &[0x00]);
    assert_canonical_decode!(CustomInquiry, &[0xA5]);
}

#[test]
fn digital_on_is_03_is_true_on_both_public_decode_paths() {
    let payload = [0x03];
    assert_canonical_decode!(BoolInquiry, &payload);
    assert!(matches!(
        BoolInquiry.parse_response(&payload),
        Ok(InquiryData::Digital { on: true })
    ));
    assert!(matches!(
        decoded_inquiry(&BoolInquiry, &payload),
        Ok(InquiryData::Digital { on: true })
    ));
}

#[test]
fn extended_zoom_position_is_accepted_on_both_public_decode_paths() {
    let payload = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
    assert_canonical_decode!(PositionInquiry, &payload);
    assert!(matches!(
        PositionInquiry.parse_response(&payload),
        Ok(InquiryData::ZoomPosition { position: 0x0123 })
    ));
    assert!(matches!(
        decoded_inquiry(&PositionInquiry, &payload),
        Ok(InquiryData::ZoomPosition { position: 0x0123 })
    ));
}

#[test]
fn explicit_selector_overrides_are_shared_by_both_public_decode_paths() {
    let custom_payload = [0xA5];
    assert_canonical_decode!(CustomInquiry, &custom_payload);
    assert!(matches!(
        CustomInquiry.parse_response(&custom_payload),
        Ok(InquiryData::Power { on: true })
    ));
    assert!(matches!(
        decoded_inquiry(&CustomInquiry, &custom_payload),
        Ok(InquiryData::Power { on: true })
    ));
    assert!(matches!(
        CustomTypedInquiry.parse_response(&custom_payload),
        Ok(InquiryData::Power { on: true })
    ));
    assert!(CustomTypedInquiry
        .decoder()
        .decode(&custom_payload)
        .expect("the typed decoder must receive Custom's shared response"),);

    let convention_payload = [0x02];
    assert_canonical_decode!(BoolConventionDataVariantInquiry, &convention_payload);
    assert!(matches!(
        BoolConventionDataVariantInquiry.parse_response(&convention_payload),
        Ok(InquiryData::Digital { on: true })
    ));
    assert!(matches!(
        decoded_inquiry(&BoolConventionDataVariantInquiry, &convention_payload),
        Ok(InquiryData::Digital { on: true })
    ));
}
