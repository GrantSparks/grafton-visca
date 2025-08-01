// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Procedural macros for the grafton-visca library
//!
//! This crate provides derive macros to simplify common patterns
//! in VISCA command implementations.

#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/grafton-visca/0.5.0")]

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod inquiry_command;
mod parser_templates;
mod value_macros;

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
