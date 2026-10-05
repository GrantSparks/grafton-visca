//! One contract for every macro-generated checked newtype and derive literal.
//!
//! `visca_range_type!` and `#[derive(ViscaValue)]` expand through one
//! generator, so a value outside a declared range is reported identically,
//! `MIN`/`MAX` are typed as the newtype itself, and `value()` is a `const fn`
//! taking `self`. The derive attribute parsers share one integer-literal
//! parser, so every Rust radix is accepted for VISCA byte fields.

use grafton_visca::{
    command::VISCA_TERMINATOR, CameraId, Error, Request, ViscaInquiry, ViscaValue,
};

grafton_visca::visca_range_type! {
    /// Range wrapper declared through the public declarative macro.
    MacroRange: u8 {
        min: 2,
        max: 4
    }
}

/// Range wrapper declared through the derive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "2", max = "4")]
struct DerivedRange(u8);

grafton_visca::visca_range_type! {
    /// Signed range wrapper declared through the public declarative macro.
    SignedMacroRange: i16 {
        min: -300,
        max: 300
    }
}

/// Signed range wrapper declared through the derive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "-300", max = "300")]
struct SignedDerivedRange(i16);

// Both generators expose `MIN`/`MAX` as `Self` and `value()` as a by-value
// `const fn`; these items only compile when that holds for each of them.
const MACRO_MIN: MacroRange = MacroRange::MIN;
const MACRO_MAX: u8 = MacroRange::MAX.value();
const DERIVED_MIN: DerivedRange = DerivedRange::MIN;
const DERIVED_MAX: u8 = DerivedRange::MAX.value();

fn assert_out_of_range(error: &Error, parameter: &str, value: i32, min: i32, max: i32) {
    match error {
        Error::ParameterOutOfRange {
            parameter: reported,
            value: reported_value,
            min: reported_min,
            max: reported_max,
            ..
        } => {
            assert_eq!(*reported, parameter);
            assert_eq!(
                (*reported_value, *reported_min, *reported_max),
                (value, min, max)
            );
        }
        other => panic!("expected ParameterOutOfRange for {parameter}, got {other:?}"),
    }
}

#[test]
fn both_newtype_generators_report_out_of_range_values_identically() {
    assert_out_of_range(
        &MacroRange::new(5).expect_err("above the macro range"),
        "MacroRange",
        5,
        2,
        4,
    );
    assert_out_of_range(
        &DerivedRange::new(5).expect_err("above the derived range"),
        "DerivedRange",
        5,
        2,
        4,
    );
    assert_out_of_range(
        &MacroRange::try_from(1).expect_err("below the macro range"),
        "MacroRange",
        1,
        2,
        4,
    );
    assert_out_of_range(
        &DerivedRange::try_from(1).expect_err("below the derived range"),
        "DerivedRange",
        1,
        2,
        4,
    );
    assert_out_of_range(
        &SignedMacroRange::new(-301).expect_err("below the signed macro range"),
        "SignedMacroRange",
        -301,
        -300,
        300,
    );
    assert_out_of_range(
        &SignedDerivedRange::new(-301).expect_err("below the signed derived range"),
        "SignedDerivedRange",
        -301,
        -300,
        300,
    );
}

#[test]
fn both_newtype_generators_share_bounds_constants_and_accessors() {
    assert_eq!(MACRO_MIN.value(), 2);
    assert_eq!(MACRO_MAX, 4);
    assert_eq!(DERIVED_MIN.value(), 2);
    assert_eq!(DERIVED_MAX, 4);
    assert_eq!(MacroRange::new(3).ok().map(MacroRange::value), Some(3));
    assert_eq!(DerivedRange::new(3).ok().map(DerivedRange::value), Some(3));
    assert_eq!(u8::from(MacroRange::MAX), 4);
    assert_eq!(u8::from(DerivedRange::MAX), 4);
    assert_eq!(SignedMacroRange::MIN.value(), -300);
    assert_eq!(SignedDerivedRange::MIN.value(), -300);
}

/// Sparse wrapper declared through the derive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(valid_values = "[0x01, 0x03]")]
struct Sparse(u8);

// The generator adds nothing to the type beyond its documented API, so a
// caller's own item of the name it uses internally does not collide.
impl Sparse {
    const VALID_VALUES: &'static str = "caller-owned";
}

#[test]
fn sparse_derived_values_report_the_declared_set() {
    assert_eq!(Sparse::MIN.value(), 1);
    assert_eq!(Sparse::MAX.value(), 3);
    assert_eq!(Sparse::VALID_VALUES, "caller-owned");
    match Sparse::new(2).expect_err("value outside the set") {
        Error::InvalidParameter {
            parameter, reason, ..
        } => {
            assert_eq!(parameter, "Sparse");
            assert_eq!(reason, "must be one of [1, 3]");
        }
        other => panic!("expected InvalidParameter, got {other:?}"),
    }
}

// A range's `new` is a `const fn` for both generators.
const MACRO_CHECKED: Result<MacroRange, Error> = MacroRange::new(3);
const DERIVED_CHECKED: Result<DerivedRange, Error> = DerivedRange::new(3);

#[test]
fn range_constructors_are_const_and_checked() {
    assert_eq!(MACRO_CHECKED.ok().map(MacroRange::value), Some(3));
    assert_eq!(DERIVED_CHECKED.ok().map(DerivedRange::value), Some(3));
}

#[test]
fn out_of_range_display_names_an_inclusive_range() {
    assert_eq!(
        MacroRange::new(5).expect_err("above range").to_string(),
        "Parameter out of range: MacroRange = 5 (valid range: 2..=4)"
    );
    assert_eq!(
        SignedDerivedRange::new(301)
            .expect_err("above range")
            .to_string(),
        "Parameter out of range: SignedDerivedRange = 301 (valid range: -300..=300)"
    );
}

/// Hex-formatted wrapper declared through the derive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    min = "0",
    max = "0x1FF",
    display_format = "hex",
    display_prefix = "Code"
)]
struct HexCode(u16);

#[test]
fn hex_display_pads_to_two_digits() {
    let format = |raw| HexCode::new(raw).map(|code| code.to_string()).ok();
    assert_eq!(format(0x05).as_deref(), Some("Code 0x05"));
    assert_eq!(format(0x1A).as_deref(), Some("Code 0x1a"));
    assert_eq!(format(0x1FF).as_deref(), Some("Code 0x1ff"));
}

#[test]
fn camera_error_codes_print_two_hex_digits() {
    assert_eq!(
        Error::from_code(0x06).to_string(),
        "Unknown error code: 0x06"
    );
    assert_eq!(
        Error::from_code(0x5A).to_string(),
        "Unknown error code: 0x5A"
    );
}

/// Opcode and subcode written in binary and octal radix.
#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0b0100_0111, subcode = 0o4, response = ZoomPosition)]
struct RadixInquiry;

#[test]
fn inquiry_derive_accepts_every_integer_literal_radix() {
    let mut buffer = [0u8; <RadixInquiry as Request>::MAX_SIZE];
    let written = RadixInquiry
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("five-byte inquiry");
    assert_eq!(
        &buffer[..written],
        &[0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR]
    );
}

#[test]
fn exposure_compensation_uses_the_single_out_of_range_contract() {
    use grafton_visca::types::ExposureCompensationLevel;

    assert_eq!(ExposureCompensationLevel::MIN.value(), -7);
    assert_eq!(ExposureCompensationLevel::MAX.value(), 7);
    assert_out_of_range(
        &ExposureCompensationLevel::new(8).expect_err("above range"),
        "ExposureCompensationLevel",
        8,
        -7,
        7,
    );
    assert_out_of_range(
        &ExposureCompensationLevel::new(-8).expect_err("below range"),
        "ExposureCompensationLevel",
        -8,
        -7,
        7,
    );
}
