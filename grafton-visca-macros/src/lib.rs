// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Attributes for checked VISCA values and the inquiry request derive.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod attr;
mod capability_encodings;
mod crate_path;
mod inquiry_command;
mod noun_signature;
mod value;

/// Declares a checked scalar with a uniform value and wire-codec contract.
///
/// Apply to a private-field, non-generic tuple struct. `min` and `max` are
/// inclusive bounds; `valid_values = [..]` declares a finite set
/// instead. `display` is a format string or formatter function path. The
/// optional `wire = biased(offset, u8)` (or `centered(offset, u8)`) declares
/// a shifted integer wire representation. The wire type defaults to `u8`.
/// `f32` and `f64` values reject non-finite input and bounds, normalize signed
/// zero, and have consistent total ordering and hashing. They use identity
/// wire representations. Float bounds may be literals or constants.
///
/// The attribute emits the standard value derives, checked `const fn new`,
/// `value`, `MIN`, `MAX`, scalar conversions, `Display`, and `WireValue`.
/// The defining library's enabled serde, schemars and ts-rs features control
/// their helper derives, including when the caller uses different features.
/// JSON Schemas retain the newtype's metadata and references. Integer schemas
/// describe checked bounds or exact finite sets in serialized semantic units;
/// float range schemas retain their declared bounds. Profile limits and biased
/// wire codes do not change the integer schema domain.
/// A re-export-only facade can be selected with `crate = facade::visca`.
#[proc_macro_attribute]
pub fn value(args: TokenStream, item: TokenStream) -> TokenStream {
    value::attribute(value::Kind::Scalar, args.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declares a value enum with one canonical `WireValue` codec.
///
/// Unit variants use explicit integer discriminants or `#[wire(code)]`.
/// The wire type defaults to `u8`; use `wire = (u8, u8)` for tuple codes.
/// One tuple variant may hold a checked value that supplies the remaining
/// wire domain. Its payload must be a `#[value]` scalar whose wire domain
/// excludes every active unit variant's code; overlap and primitive payloads
/// are compile errors. `#[value(display = "label")]` sets a variant's display text.
/// The attribute emits the standard derives, `Display`, helper derives and
/// `WireValue`, without inherent wire-codec aliases or primitive conversions.
#[proc_macro_attribute]
pub fn value_enum(args: TokenStream, item: TokenStream) -> TokenStream {
    value::attribute(value::Kind::Enum, args.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Expands a value after the main library has selected its helper features.
#[doc(hidden)]
#[proc_macro]
pub fn __value(input: TokenStream) -> TokenStream {
    value::expand(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generates internal profile encoding data from the typed-support registry.
#[doc(hidden)]
#[proc_macro]
pub fn __capability_encodings(input: TokenStream) -> TokenStream {
    capability_encodings::expand(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generates the closed noun-signature value imports for facade preludes.
#[doc(hidden)]
#[proc_macro]
pub fn __noun_signature_types(input: TokenStream) -> TokenStream {
    noun_signature::expand(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive macro for generating typed inquiry request implementations with optional
/// inherent response parsing.
///
/// This macro eliminates boilerplate by automatically generating the typed
/// [`Request`](https://docs.rs/grafton-visca/latest/grafton_visca/request/trait.Request.html)
/// and [`Inquiry`](https://docs.rs/grafton-visca/latest/grafton_visca/request/trait.Inquiry.html)
/// implementations with exact `MAX_SIZE` and zero-allocation `write_into()`.
/// When a `parser` attribute is provided for a built-in response, it also
/// generates a `parse_response()` method. Both that inherent method and
/// [`Inquiry::decoder`](https://docs.rs/grafton-visca/latest/grafton_visca/request/trait.Inquiry.html#tymethod.decoder)
/// use one shared generated decoder. Selectors that identify a built-in table
/// shape delegate to the canonical decoder for `response`; selectors with an
/// established transformation, such as `Custom` and `BoolConvention`, retain
/// that behavior on both paths.
///
/// # Basic Usage
///
/// ```rust,ignore
/// use grafton_visca::{types::CameraId, request::Request, ViscaInquiry};
///
/// #[derive(ViscaInquiry, Debug, Copy, Clone)]
/// #[visca(opcode = 0x00, response = Power)]
/// struct PowerInquiry;
///
/// let mut buffer = [0u8; <PowerInquiry as Request>::MAX_SIZE];
/// let len = PowerInquiry.write_into(CameraId::CAMERA_1, &mut buffer)?;
/// assert_eq!(
///     &buffer[..len],
///     &[0x81, 0x09, 0x04, 0x00, grafton_visca::wire::VISCA_TERMINATOR]
/// );
/// # Ok::<(), grafton_visca::Error>(())
/// ```
///
/// # Response parsing
///
/// The `response` kind selects the canonical inquiry table, including its
/// profile-aware position framing. Every built-in kind gets an inherent
/// `parse_response()` method and the same decoder through `Inquiry`.
///
/// ```rust,ignore
/// #[derive(ViscaInquiry, Debug, Copy, Clone)]
/// #[visca(opcode = 0x12, subcode = 0x06, response = PanTiltPosition)]
/// struct PositionInquiry;
/// ```
///
/// Use `parser = Custom, parse_with = path` for an explicit custom parser.
/// `data_variant` is available only for a custom parser with `typed_response`.
/// Typed response paths resolve exactly as written in the deriving scope.
/// Built-in scalar fields already contain checked values; use the default
/// identity conversion when `typed_response` names that same checked type.
/// Constructor modes support downstream projections and custom parsers.
/// `typed_constructor = new_u8` checks narrowing and rejects overflow with
/// `InvalidResponse`. Other parser selectors are unsupported: omit `parser`
/// for canonical decoding, or use `Custom` and the checked payload helpers.
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
///
/// `Debug`, `Copy`, and `Clone` are optional; the generated request and inquiry
/// implementations do not require them.
#[proc_macro_derive(ViscaInquiry, attributes(visca))]
pub fn derive_visca_inquiry(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(inquiry_command::derive_visca_inquiry_impl(input))
}
