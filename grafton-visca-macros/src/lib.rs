// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Procedural macros for the grafton-visca library
//!
//! This crate provides derive and attribute macros to simplify common patterns
//! in VISCA command implementations.

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod bounded_macros;
mod command_macros;
mod inquiry_command;
mod method_macros;
mod parser_templates;
mod position_macros;
mod speed_macros;
mod test_macros;
mod value_macros;

#[proc_macro_attribute]
pub fn visca_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_method(attr, item)
}

#[proc_macro_attribute]
pub fn visca_method_custom(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_method_custom(attr, item)
}

#[proc_macro_attribute]
pub fn visca_camera_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_camera_method(attr, item)
}

#[proc_macro_attribute]
pub fn visca_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    command_macros::visca_command(attr, item)
}

#[proc_macro_attribute]
pub fn visca_command_variants(attr: TokenStream, item: TokenStream) -> TokenStream {
    command_macros::visca_command_variants(attr, item)
}

#[proc_macro_attribute]
pub fn visca_method_generic(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_method_generic(attr, item)
}

#[proc_macro_attribute]
pub fn visca_inquiry(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_inquiry(attr, item)
}

#[proc_macro_attribute]
pub fn visca_position_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    position_macros::visca_position_command(attr, item)
}

#[proc_macro_attribute]
pub fn visca_speed_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    speed_macros::visca_speed_command(attr, item)
}
#[proc_macro_attribute]
pub fn visca_bounded_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    bounded_macros::visca_bounded_command(attr, item)
}
#[proc_macro_derive(ViscaValue, attributes(visca_value))]
pub fn derive_visca_value(input: TokenStream) -> TokenStream {
    value_macros::derive_visca_value(input)
}

#[proc_macro_attribute]
pub fn visca_fallible_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_fallible_method(attr, item)
}

#[proc_macro_attribute]
pub fn visca_mock_transport(attr: TokenStream, item: TokenStream) -> TokenStream {
    test_macros::visca_mock_transport(attr, item)
}
/// Generate validation tests for VISCA commands.
///
/// This macro generates comprehensive test suites that validate:
/// - Command byte sequences against documentation
/// - Parameter bounds for all camera profiles
/// - Response parsing for all command types
/// - Error cases and edge conditions
///
/// # Example
///
/// ```rust,ignore
/// #[visca_test_suite]
/// mod zoom_command_tests {
///     use super::*;
///     
///     #[test_command(
///         command = "ZoomCommand::Direct(ZoomPosition::new(0x4000)?)",
///         expected_bytes = "[0x81, 0x01, 0x04, 0x47, 0x04, 0x00, 0x00, 0x00, 0xFF]",
///         profiles = ["PTZOpticsG2", "SonyEVID70"]
///     )]
///     fn test_zoom_direct() {}
///     
///     #[test_bounds(
///         parameter = "zoom_position",
///         min = "0x0000",
///         max = "0x7000",
///         profiles = ["PTZOpticsG2"]
///     )]
///     fn test_zoom_bounds() {}
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_test_suite(attr: TokenStream, item: TokenStream) -> TokenStream {
    test_macros::visca_test_suite(attr, item)
}


/// Derive macro for generating InquiryCommand implementations with parser support
///
/// This macro eliminates boilerplate by automatically generating the `Command` trait
/// implementation with `to_bytes()`, `response_type()`, and `command_category()` methods.
/// When parser attributes are provided, it also generates a `parse_response()` method.
///
/// # Basic Usage
///
/// ```rust,ignore
/// #[derive(Debug, InquiryCommand, PartialEq)]
/// enum MyInquiry {
///     #[visca(0x00, response = Power)]
///     Power,
///     
///     #[visca(0x47, response = ZoomPosition)]
///     ZoomPos,
///     
///     #[visca(0x12, subcategory = 0x06, response = PanTiltPosition)]
///     PanTiltPos,
/// }
/// ```
///
/// # With Response Parsing
///
/// Add parser attributes to automatically generate response parsing:
///
/// ```rust,ignore
/// #[derive(Debug, InquiryCommand, PartialEq)]
/// enum MyInquiry {
///     #[visca(0x00, response = Power, parser = "bool")]
///     Power,
///
///     #[visca(0x47, response = ZoomPosition, parser = "position")]
///     ZoomPos,
///
///     #[visca(0xA1, response = Luminance, parser = "byte")]
///     Luminance,
///
///     #[visca(0x44, response = RedGain, parser = "offset", field = "gain", offset = 10)]
///     RedGain,
/// }
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
/// The `response` attribute must reference existing variants in the `ResponseType`
/// and `InquiryResponse` enums.
#[proc_macro_derive(InquiryCommand, attributes(visca))]
pub fn derive_inquiry_command(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(inquiry_command::derive_inquiry_command_impl(input))
}
