// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Procedural macros for the grafton-visca library
//!
//! This crate provides derive macros to simplify common patterns
//! in VISCA command implementations.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/grafton-visca/0.5.0")]

use syn::{parse_macro_input, DeriveInput};

use proc_macro::TokenStream;

mod inquiry_command;
mod parser_templates;
mod value_macros;
mod visca_encode;
mod visca_enum;

/// Derive macro for implementing ViscaValue trait for command value types
///
/// This macro automatically generates the `ViscaValue` trait implementation
/// for types that represent VISCA command values, providing methods for
/// converting to and from byte representations.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca_macros::ViscaValue;
///
/// #[derive(ViscaValue, Debug, Copy, Clone)]
/// #[visca_value(bytes = 2)]
/// struct ZoomPosition(u16);
/// ```
#[proc_macro_derive(ViscaValue, attributes(visca_value))]
pub fn derive_visca_value(input: TokenStream) -> TokenStream {
    value_macros::derive_visca_value(input)
}

/// Derive macro for generating InquiryCommand implementations with parser support
///
/// This macro eliminates boilerplate by automatically generating the `Command` trait
/// implementation with `to_bytes()`, `response_type()`, and `command_category()` methods,
/// as well as a `From` conversion to the `InquiryCommand` enum.
/// When parser attributes are provided, it also generates a `parse_response()` method.
///
/// # Basic Usage
///
/// ```rust,ignore
/// use grafton_visca_macros::InquiryCommand;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x00, response = "Power", inquiry_variant = "Power")]
/// struct PowerInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x47, response = "ZoomPosition", inquiry_variant = "ZoomPosition")]
/// struct ZoomPositionInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x12, sub_command = 0x06, response = "PanTiltPosition", inquiry_variant = "PanTiltPosition")]
/// struct PanTiltPositionInquiry;
/// ```
///
/// # With Response Parsing
///
/// Add parser attributes to automatically generate response parsing:
///
/// ```rust,ignore
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x00, response = "Power", inquiry_variant = "Power", parser = "bool")]
/// struct PowerInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x47, response = "ZoomPosition", inquiry_variant = "ZoomPosition", parser = "position")]
/// struct ZoomPositionInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0xA1, response = "Luminance", inquiry_variant = "Luminance", parser = "byte")]
/// struct LuminanceInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x44, response = "RedGain", inquiry_variant = "RedGain", parser = "offset", field = "gain", offset = 10)]
/// struct RedGainInquiry;
/// ```
///
/// # Supported Parser Types
///
/// - `"bool"` - Boolean values (0x02 = true, 0x03 = false)
/// - `"byte"` - Direct byte value
/// - `"position"` - 4-nibble position value (converts to u16)
/// - `"nibble"` - Extended nibble encoding
/// - `"offset"` - Byte value with offset subtraction
/// - `"flags"` - Bit flags (for image flip)
/// - `"mode"` - Enum value parsing
/// - `"pan_tilt"` - Special parser for pan/tilt positions
///
/// # Requirements
///
/// - The struct must have the `#[visca(...)]` attribute with required fields
/// - The `response` attribute must reference existing variants in the `ResponseType` enum
/// - The `inquiry_variant` must reference existing variants in the `InquiryCommand` enum
/// - The struct should implement `Debug`, `Copy`, and `Clone` for full compatibility
#[proc_macro_derive(InquiryCommand, attributes(visca))]
pub fn derive_inquiry_command(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(inquiry_command::derive_inquiry_command_impl(input))
}

/// Derive macro for automatic VISCA command encoding
///
/// This macro automatically generates the `ViscaEncode` trait implementation
/// for commands, eliminating boilerplate code for byte sequence encoding.
///
/// # Basic Usage
///
/// ```rust,ignore
/// use grafton_visca_macros::ViscaEncode;
///
/// #[derive(ViscaEncode, Debug, Copy, Clone)]
/// #[visca_encode(response = "Power", max_size = 6, timeout = "Quick")]
/// struct PowerOnCommand;
/// ```
///
/// # For Enums
///
/// ```rust,ignore
/// #[derive(ViscaEncode, Debug, Copy, Clone)]
/// #[visca_encode(max_size = 6, timeout = "Quick")]
/// enum PowerCommand {
///     #[visca_bytes(0x81, 0x01, 0x04, 0x00, 0x02)]
///     On,
///     #[visca_bytes(0x81, 0x01, 0x04, 0x00, 0x03)]
///     Standby,
/// }
/// ```
///
/// # Attributes
///
/// ## Type-level attributes (`#[visca_encode(...)]`):
/// - `response` - The ResponseType variant name (for inquiries)
/// - `max_size` - Maximum command size in bytes (default: 32)
/// - `timeout` - CommandCategory variant name (default: "Custom")
/// - `prefix` - Common prefix bytes for all variants
///
/// ## Variant-level attributes (`#[visca_bytes(...)]`):
/// - List of byte values for the command
///
/// # Generated Implementation
///
/// The macro generates a complete `ViscaEncode` trait implementation with:
/// - Proper type-state pattern using CommandBuilder
/// - Automatic terminator handling
/// - Camera ID injection
/// - Buffer size validation
#[proc_macro_derive(ViscaEncode, attributes(visca_encode, visca_bytes))]
pub fn derive_visca_encode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(visca_encode::derive_visca_encode_impl(input))
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
/// - Discriminant values must be valid u8 values (0-255)
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
/// #[visca_enum(error_type = MyError, exhaustive = false)]
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
/// ## Enum-level attributes:
/// - `error_type` - Custom error type for TryFrom (default: `crate::error::Error`)
/// - `exhaustive` - Whether to generate exhaustive match (default: true)
///
/// ## Variant-level attributes:
/// - `name` - Custom name to use in error messages
/// - `skip` - Skip this variant in `TryFrom<u8>` (but include in `From<Enum>`)
///
/// # Error Handling
///
/// The generated `TryFrom<u8>` implementation returns an error with a descriptive
/// message listing all valid values when an invalid u8 is provided.
#[proc_macro_derive(ViscaEnum, attributes(visca_enum))]
pub fn derive_visca_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(visca_enum::derive_visca_enum_impl(input))
}
