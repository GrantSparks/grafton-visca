use grafton_visca_test_support::{active_grafton_visca_features, compile_fail};

use grafton_visca::{
    command::{ExposureMode, InquiryData},
    ViscaInquiry,
};

// Keep generated derive code independent of names supplied by the downstream
// crate. The canonical parser must be reached through absolute crate paths.
// The derives below expand in the scope of these shadows; a derive that
// reached one would not compile. `downstream_shadows_are_in_scope` proves the
// shadows really are what these names resolve to here.
struct Result;
struct TryFrom;
enum Ok {}
enum Err {}

macro_rules! vec {
    (@shadow) => {
        "downstream vec!"
    };
    ($($tokens:tt)*) => {
        compile_error!("derive output used the downstream vec! macro")
    };
}

macro_rules! format {
    (@shadow) => {
        "downstream format!"
    };
    ($($tokens:tt)*) => {
        compile_error!("derive output used the downstream format! macro")
    };
}

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Bool)]
struct StrictBoolInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x01, response = GainLimit, parser = DirectByte)]
struct StrictDirectByteInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x02, response = NoiseReduction2D, parser = Byte)]
struct StrictByteInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x03, response = FlipState, parser = Flags)]
struct StrictFlagsInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x04,
    response = ExposureMode,
    parser = Mode,
    value_type = ExposureMode
)]
struct StrictModeInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x05, response = TallyStatus, parser = TallyStatus)]
struct StrictTallyInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x06, response = SharpnessMode, parser = SharpnessMode)]
struct StrictSharpnessInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x07, response = Gamma, parser = Gamma)]
struct StrictGammaInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x08,
    response = AutoWhiteBalanceSensitivity,
    parser = AutoWbSensitivity
)]
struct StrictAwbInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x09, response = ZoomPosition, parser = Position)]
struct StrictPositionInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x0A,
    response = Iris,
    parser = ExtendedNibble,
    field = position
)]
struct StrictExtendedInquiry;

#[test]
fn generated_parser_selectors_use_the_canonical_wire_lengths() {
    assert!(matches!(
        StrictBoolInquiry.parse_response(&[0x02]),
        ::core::result::Result::Ok(InquiryData::Power { on: true })
    ));
    assert!(StrictBoolInquiry.parse_response(&[0x02, 0x00]).is_err());

    assert!(matches!(
        StrictDirectByteInquiry.parse_response(&[0x07]),
        ::core::result::Result::Ok(InquiryData::GainLimit { limit: 0x07 })
    ));
    assert!(StrictDirectByteInquiry
        .parse_response(&[0x07, 0x00])
        .is_err());

    assert!(matches!(
        StrictByteInquiry.parse_response(&[0x07]),
        ::core::result::Result::Ok(InquiryData::NoiseReduction2D { level: 0x07 })
    ));
    assert!(StrictByteInquiry.parse_response(&[0x07, 0x00]).is_err());

    assert!(StrictFlagsInquiry.parse_response(&[0x03]).is_ok());
    assert!(StrictFlagsInquiry.parse_response(&[0x03, 0x00]).is_err());

    assert!(matches!(
        StrictModeInquiry.parse_response(&[0x00]),
        ::core::result::Result::Ok(InquiryData::ExposureMode {
            mode: ExposureMode::Auto
        })
    ));
    assert!(StrictModeInquiry.parse_response(&[0x00, 0x00]).is_err());

    assert!(StrictTallyInquiry.parse_response(&[0x03, 0x02]).is_ok());
    assert!(StrictTallyInquiry
        .parse_response(&[0x03, 0x02, 0x00])
        .is_err());

    assert!(StrictSharpnessInquiry.parse_response(&[0x02]).is_ok());
    assert!(StrictSharpnessInquiry
        .parse_response(&[0x02, 0x00])
        .is_err());

    assert!(StrictGammaInquiry.parse_response(&[0x04]).is_ok());
    assert!(StrictGammaInquiry.parse_response(&[0x04, 0x00]).is_err());

    assert!(StrictAwbInquiry.parse_response(&[0x00]).is_ok());
    assert!(StrictAwbInquiry.parse_response(&[0x00, 0x00]).is_err());
}

#[test]
fn generated_nibble_templates_reject_bad_nibbles_and_trailing_bytes() {
    assert!(StrictPositionInquiry
        .parse_response(&[0x00, 0x01, 0x02, 0x03])
        .is_ok());
    assert!(StrictPositionInquiry
        .parse_response(&[0x10, 0x01, 0x02, 0x03])
        .is_err());
    assert!(StrictPositionInquiry
        .parse_response(&[0x00, 0x01, 0x02, 0x03, 0x00])
        .is_err());

    assert!(StrictExtendedInquiry.parse_response(&[0x00, 0x05]).is_ok());
    assert!(StrictExtendedInquiry.parse_response(&[0x10, 0x05]).is_err());
    assert!(StrictExtendedInquiry
        .parse_response(&[0x00, 0x05, 0x00])
        .is_err());
}

#[test]
fn renamed_downstream_names_do_not_capture_generated_parser_code() {
    let features = active_grafton_visca_features!();
    compile_fail::assert_compile_pass_fixture_paths(
        &["tests/api_contract/pass/derive_macro_mode_hygiene.rs"],
        &features,
    );
}

/// The shadows above are what the unqualified names resolve to in the scope
/// where the derives expand. Each line compiles only against the shadow: the
/// prelude's `Result` is generic, its `TryFrom` is a trait, its `Ok`/`Err` are
/// variants rather than types, and its `vec!`/`format!` have no `@shadow`
/// form.
#[test]
fn downstream_shadows_are_in_scope() {
    assert_eq!(std::mem::size_of_val(&Result), 0);
    assert_eq!(std::mem::size_of_val(&TryFrom), 0);
    assert_eq!(std::mem::size_of::<Option<Ok>>(), 0);
    assert_eq!(std::mem::size_of::<Option<Err>>(), 0);
    assert_eq!(vec!(@shadow), "downstream vec!");
    assert_eq!(format!(@shadow), "downstream format!");
}
