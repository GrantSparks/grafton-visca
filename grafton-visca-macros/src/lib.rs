// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Procedural macros for the grafton-visca library
//!
//! This crate provides derive macros to simplify common patterns
//! in VISCA command implementations.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use syn::{parse_macro_input, DeriveInput};

use proc_macro::TokenStream;

mod attr;
mod bounded;
mod crate_path;
mod inquiry_command;
mod newtype_decl;
mod visca_enum;

/// Internal declaration adapter used by `grafton_visca::visca_range_type!`
/// and by the main crate's own checked value types.
///
/// This is exported only because a declarative macro must invoke it after the
/// main crate has selected its enabled helper features. It is not a supported
/// standalone API.
#[doc(hidden)]
#[proc_macro]
pub fn __grafton_visca_newtype(input: TokenStream) -> TokenStream {
    newtype_decl::expand(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive macro for checked value newtypes.
///
/// Applied to a tuple struct with one field, it generates the same
/// constructor, bounds, accessor and error contract as
/// `grafton_visca::visca_range_type!`, because both expand through one
/// generator:
///
/// - `MIN` and `MAX`, typed as `Self`;
/// - `new(value) -> Result<Self, grafton_visca::Error>`, which checks the
///   declared domain (a `const fn` for a `min`/`max` range);
/// - `pub const fn value(self)`, returning the raw value;
/// - `TryFrom<inner>` and `From<Self> for inner`;
/// - `Display`, formatted by `display_format` and `display_prefix`.
///
/// It does not generate VISCA byte encoding or decoding APIs, and it does not
/// add serde, schemars or ts-rs derives; add those on the struct as needed.
///
/// # Attributes
///
/// Each key may appear once across all `#[visca_value(...)]` attributes.
///
/// - `min` and `max`: the inclusive range, each written as an unsuffixed
///   integer literal string. A value outside it is reported as
///   `Error::ParameterOutOfRange`, whose bounds are `i32`, so the inner type
///   must convert into `i32` without loss (`u8`, `u16`, `i8`, `i16` or
///   `i32`), and `min` must not exceed `max`; both are checked at compile
///   time.
/// - `valid_values`: a non-empty array literal string, instead of `min` and
///   `max`. A value outside it is reported as `Error::InvalidParameter`.
/// - `display_format`: `"decimal"` (the default), `"hex"` (`0x05`, at least
///   two digits) or `"binary"`.
/// - `display_prefix`: text written before the value.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca_macros::ViscaValue;
///
/// #[derive(ViscaValue, Debug, Copy, Clone)]
/// #[visca_value(min = "0x0000", max = "0x4000")]
/// struct ZoomPosition(u16);
/// ```
#[proc_macro_derive(ViscaValue, attributes(visca_value))]
pub fn derive_visca_value(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    bounded::derive_visca_value(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive macro for generating typed inquiry request implementations with optional
/// inherent response parsing.
///
/// This macro eliminates boilerplate by automatically generating the typed
/// [`Request`](https://docs.rs/grafton-visca/latest/grafton_visca/trait.Request.html)
/// and [`Inquiry`](https://docs.rs/grafton-visca/latest/grafton_visca/trait.Inquiry.html)
/// implementations with exact `MAX_SIZE` and zero-allocation `write_into()`.
/// When a `parser` attribute is provided for a built-in response, it also
/// generates a `parse_response()` method. Both that inherent method and
/// [`Inquiry::decoder`](https://docs.rs/grafton-visca/latest/grafton_visca/trait.Inquiry.html#tymethod.decoder)
/// use one shared generated decoder. Selectors that identify a built-in table
/// shape delegate to the canonical decoder for `response`; selectors with an
/// established transformation, such as `Custom` and `BoolConvention`, retain
/// that behavior on both paths.
///
/// # Basic Usage
///
/// ```rust,ignore
/// use grafton_visca::{CameraId, Request, ViscaInquiry};
///
/// #[derive(ViscaInquiry, Debug, Copy, Clone)]
/// #[visca(opcode = 0x00, response = Power)]
/// struct PowerInquiry;
///
/// let mut buffer = [0u8; <PowerInquiry as Request>::MAX_SIZE];
/// let len = PowerInquiry.write_into(CameraId::CAMERA_1, &mut buffer)?;
/// assert_eq!(
///     &buffer[..len],
///     &[0x81, 0x09, 0x04, 0x00, grafton_visca::command::VISCA_TERMINATOR]
/// );
/// # Ok::<(), grafton_visca::Error>(())
/// ```
///
/// # With Response Parsing
///
/// Add a parser attribute to generate the inherent `parse_response()`
/// convenience method. Its `response` kind selects the canonical decoder:
///
/// ```rust,ignore
/// #[derive(ViscaInquiry, Debug, Copy, Clone)]
/// #[visca(opcode = 0x00, response = Power, parser = Bool)]
/// struct PowerInquiry;
///
/// #[derive(ViscaInquiry, Debug, Copy, Clone)]
/// #[visca(opcode = 0x47, response = ZoomPosition, parser = Position)]
/// struct ZoomPositionInquiry;
///
/// #[derive(ViscaInquiry, Debug, Copy, Clone)]
/// #[visca(opcode = 0x12, subcode = 0x06, response = PanTiltPosition, parser = PanTilt)]
/// struct PanTiltPositionInquiry;
///
/// #[derive(ViscaInquiry, Debug, Copy, Clone)]
/// #[visca(opcode = 0x4F, response = Hue, parser = LastNibble, field = hue)]
/// struct HueInquiry;
/// ```
///
/// # Parser Selectors
///
/// `parser` is the current way to request the inherent `parse_response()`
/// method. Two kinds of selector exist:
///
/// - Shape selectors (`Bool`, `DirectByte`, `Byte`, `Position`, `Flags`,
///   `BitFlags`, `PanTilt`, `TallyStatus`, `SharpnessMode`, `Gamma`,
///   `AutoWbSensitivity`) name the built-in table shape of `response` and
///   decode with that table entry, including its boolean convention and
///   accepted nibble width.
/// - Transforming selectors carry their own decoding: `Custom` calls
///   `parse_with`; `BoolConvention`, `Nibble`/`ExtendedNibble`, `LastNibble`,
///   `Mode`/`ModeEnum`, `NdFilter`, `PictureEffect`, `DefogLevel` and
///   `FocusRange` apply their transformation, with `field`, `value_type` and
///   `data_variant` where the selector uses them.
///
/// Every form is evaluated by the one shared decoder.
///
/// # Requirements
///
/// - The struct must have the `#[visca(...)]` attribute with required fields
/// - Each key may appear once across all `#[visca(...)]` attributes
/// - `opcode` and `subcode` are unsuffixed integer literals in `0..=255`, in
///   any radix (`0x47`, `0o107`, `0b0100_0111` and `71` are the same byte)
/// - The `response` attribute must reference an existing `InquiryKind` variant,
///   or `Raw` for a raw custom inquiry
/// - Downstream derives use the standard five-byte inquiry form. The selected
///   `response` kind defines the accepted reply form.
/// - The struct should implement `Debug`, `Copy`, and `Clone` for full compatibility
#[proc_macro_derive(ViscaInquiry, attributes(visca))]
pub fn derive_visca_inquiry(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(inquiry_command::derive_visca_inquiry_impl(input))
}

/// Derive macro for automatic enum/u8 conversions in VISCA protocol
///
/// This macro automatically generates `TryFrom<u8>` and `From<Enum> for u8`
/// implementations for enums with explicit discriminants, eliminating boilerplate
/// code for VISCA protocol value conversions.
///
/// # Requirements
///
/// - The enum must have unit variants only (no fields)
/// - All variants must have explicit discriminant values
/// - Discriminant values must be unique
/// - Each discriminant is an unsuffixed integer literal in `0..=255`, in any
///   radix (`0x0A`, `0o12`, `0b1010` and `10` are equivalent)
///
/// # Generated Implementations
///
/// The macro generates:
/// - `TryFrom<u8>` - Converts u8 values to enum variants, returning an error for invalid values
/// - `From<Enum> for u8` - Converts enum variants to their u8 discriminant values
/// - `is_valid_discriminant(u8) -> bool` - Const function to check if a value is valid
///
/// # Basic Example
///
/// ```rust,ignore
/// use grafton_visca_macros::ViscaEnum;
///
/// #[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
/// pub enum ExposureMode {
///     Auto = 0x00,
///     Manual = 0x03,
///     Shutter = 0x0A,
///     Iris = 0x0B,
///     Bright = 0x0D,
/// }
///
/// // The macro generates:
/// // - impl TryFrom<u8> for ExposureMode { ... }
/// // - impl From<ExposureMode> for u8 { ... }
/// // - impl ExposureMode { pub const fn is_valid_discriminant(u8) -> bool { ... } }
/// ```
///
/// # Advanced Attributes
///
/// The macro supports optional attributes for customization:
///
/// ```rust,ignore
/// #[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
/// #[visca_enum(error_type = MyError)]
/// pub enum Mode {
///     #[visca_enum(name = "Automatic Mode")]
///     Auto = 0x00,
///
///     #[visca_enum(name = "Manual Control")]
///     Manual = 0x03,
///
///     #[visca_enum(skip)]
///     _Reserved = 0xFF,  // Not included in TryFrom<u8>
/// }
/// ```
///
/// Each key may appear once across all of an item's `#[visca_enum(...)]`
/// attributes; a repeated key is a compile error naming both occurrences.
///
/// ## Enum-level attributes:
/// - `error_type` - Custom error type for TryFrom (default: `grafton_visca::Error`)
///
/// ## Variant-level attributes:
/// - `name` - Custom name to use in error messages
/// - `skip` - Skip this variant in `TryFrom<u8>` (but include in `From<Enum>`)
///
/// # Error Handling
///
/// The generated `TryFrom<u8>` implementation returns an error with a descriptive
/// message listing all valid values when an invalid u8 is provided.
/// The default error and configured types whose final path segment is `Error`
/// must provide `invalid_response(expected, actual)`: `expected` accepts a
/// `Cow<'static, str>` (or `impl Into<Cow<'static, str>>`) and `actual` is
/// `Vec<u8>`. Other configured error names keep
/// the `From<String>` convention. This constructor convention permits the
/// library's error payload to remain non-exhaustive.
#[proc_macro_derive(ViscaEnum, attributes(visca_enum))]
pub fn derive_visca_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(visca_enum::derive_visca_enum_impl(input))
}
