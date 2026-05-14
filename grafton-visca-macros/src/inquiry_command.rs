// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! ViscaInquiry derive macro implementation with parser generation support

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident};

pub fn derive_visca_inquiry_impl(input: DeriveInput) -> TokenStream {
    match &input.data {
        syn::Data::Struct(_) => {
            let struct_name = &input.ident;

            // Parse visca attributes from the struct
            let attrs = parse_visca_attributes_from_struct(&input);

            // Extract required attributes
            let response_kind = attrs
                .response_kind
                .clone()
                .expect("visca attribute must have a 'response' value");

            // Determine crate path once for consistency
            let is_internal_crate =
                std::env::var("CARGO_CRATE_NAME").unwrap_or_default() == "grafton_visca";
            let crate_path = if is_internal_crate {
                quote! { crate }
            } else {
                quote! { ::grafton_visca }
            };

            let byte_value = attrs
                .byte_value
                .expect("visca attribute must have an 'opcode' value");
            let subcategory = attrs.subcategory.unwrap_or(0x04);

            // Inside grafton-visca, inquiry structs use the crate's private
            // canonical byte constants so unusual extended inquiries stay exact.
            // Downstream derives cannot access those private constants, so they
            // generate standard VISCA inquiry bytes directly from opcode/subcode.
            let (max_size_expr, write_into_body) = if is_internal_crate {
                let constant_name = attrs
                        .constant
                        .as_ref()
                        .expect("visca attribute must have a 'constant' or 'bytes_const' value for internal inquiries");
                let constant_path = format_ident!("{}", constant_name);
                (
                    quote! { #crate_path::command::bytes::constants::inquiry::#constant_path.len() },
                    quote! {
                        let bytes = #crate_path::command::bytes::constants::inquiry::#constant_path;
                        let len = bytes.len();
                        if buffer.len() < len {
                            return Err(#crate_path::Error::BufferTooSmall {
                                required: len,
                                actual: buffer.len(),
                            });
                        }
                        buffer[..len].copy_from_slice(bytes);
                        buffer[0] = camera_id.to_address_byte();
                        Ok(len)
                    },
                )
            } else {
                (
                    quote! { 5 },
                    quote! {
                        const LEN: usize = 5;
                        if buffer.len() < LEN {
                            return Err(#crate_path::Error::BufferTooSmall {
                                required: LEN,
                                actual: buffer.len(),
                            });
                        }
                        buffer[0] = camera_id.to_address_byte();
                        buffer[1] = 0x09;
                        buffer[2] = #subcategory;
                        buffer[3] = #byte_value;
                        buffer[4] = #crate_path::command::VISCA_TERMINATOR;
                        Ok(LEN)
                    },
                )
            };

            // Generate parser implementation if parser info is provided
            let parse_response_impl = if let Some(parser_info) = &attrs.parser {
                let parser_body = generate_parser_body(&response_kind, parser_info, &crate_path);
                quote! {
                    impl #struct_name {
                        /// Parse the response data for this inquiry command
                        pub fn parse_response(&self, data: &[u8]) -> Result<#crate_path::command::InquiryData, #crate_path::Error> {
                            if data.is_empty() {
                                return Err(#crate_path::Error::invalid_response_length(1, data));
                            }
                            #parser_body
                        }
                    }
                }
            } else {
                quote! {}
            };

            // Optionally generate a typed ResponseParser impl
            let typed_impl = generate_typed_impl(struct_name, &response_kind, &crate_path, &attrs);

            let expanded = quote! {
                impl #crate_path::command::ViscaCommand for #struct_name {
                    type Response = #crate_path::command::InquiryData;
                    const MAX_SIZE: usize = #max_size_expr;
                    const TIMEOUT_CATEGORY: #crate_path::timeout::CommandCategory = #crate_path::timeout::CommandCategory::Quick;

                    fn write_into(&self, camera_id: #crate_path::CameraId, buffer: &mut [u8]) -> Result<usize, #crate_path::Error> {
                        #write_into_body
                    }

                    fn response_kind(&self) -> Option<#crate_path::command::InquiryKind> {
                        Some(#crate_path::command::InquiryKind::#response_kind)
                    }
                }


                #parse_response_impl

                #typed_impl
            };

            expanded
        }
        _ => syn::Error::new_spanned(&input, "ViscaInquiry can only be derived for structs")
            .to_compile_error(),
    }
}

#[derive(Default)]
struct ViscaAttributes {
    byte_value: Option<u8>,
    subcategory: Option<u8>,
    response_kind: Option<Ident>,
    parser: Option<ParserInfo>,
    constant: Option<String>,
    // Typed response attributes for ResponseParser impl generation:
    typed_response: Option<String>,
    typed_field: Option<String>,
    typed_constructor: Option<String>,
    typed_is_tuple: bool,
}

struct ParserInfo {
    parser_type: String,
    field_name: Option<String>,
    mode_type: Option<String>,
    custom_fn: Option<String>,
    convention: Option<String>, // "OnIs02" or "OnIs03" for bool_convention parser
    data_variant: Option<String>, // InquiryData variant if different from response (InquiryKind)
}

fn parse_visca_attributes_from_struct(input: &DeriveInput) -> ViscaAttributes {
    let mut attrs = ViscaAttributes::default();

    for attr in &input.attrs {
        if attr.path().is_ident("visca") {
            // Parse the attribute manually
            let tokens = attr
                .parse_args::<proc_macro2::TokenStream>()
                .expect("Failed to parse visca attribute");
            let token_str = tokens.to_string();

            // Split by comma and parse each part
            for part in token_str.split(',') {
                let part = part.trim();

                // Typed response attributes (must be checked before "response"/"field")
                if part.contains("typed_response") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("typed_response must have a value")
                        .trim()
                        .trim_matches('"');
                    attrs.typed_response = Some(value.to_string());
                } else if part.contains("typed_field") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("typed_field must have a value")
                        .trim()
                        .trim_matches('"');
                    attrs.typed_field = Some(value.to_string());
                } else if part.contains("typed_constructor") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("typed_constructor must have a value")
                        .trim()
                        .trim_matches('"');
                    attrs.typed_constructor = Some(value.to_string());
                } else if part.contains("typed_is_tuple") {
                    attrs.typed_is_tuple = true;
                } else if (part.contains("command") && !part.contains("sub_command"))
                    || part.contains("opcode")
                {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("command/opcode must have a value")
                        .trim();
                    if value.starts_with("0x") {
                        attrs.byte_value = Some(
                            u8::from_str_radix(value.trim_start_matches("0x"), 16)
                                .expect("command/opcode must be a valid hex u8"),
                        );
                    } else {
                        attrs.byte_value = Some(
                            value
                                .parse::<u8>()
                                .expect("command/opcode must be a valid u8"),
                        );
                    }
                } else if part.contains("response") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("response must have a value")
                        .trim()
                        .trim_matches('"');
                    // For struct attributes, response should be a simple string
                    attrs.response_kind = Some(format_ident!("{}", value));
                } else if part.contains("sub_command") || part.contains("subcode") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("sub_command/subcode must have a value")
                        .trim();
                    if value.starts_with("0x") {
                        attrs.subcategory = Some(
                            u8::from_str_radix(value.trim_start_matches("0x"), 16)
                                .expect("sub_command/subcode must be a valid hex u8"),
                        );
                    } else {
                        attrs.subcategory = Some(
                            value
                                .parse::<u8>()
                                .expect("sub_command/subcode must be a valid u8"),
                        );
                    }
                } else if part.contains("parser") {
                    let parser_value = part
                        .split('=')
                        .nth(1)
                        .expect("parser must have a value")
                        .trim()
                        .trim_matches('"');

                    let parser_info = ParserInfo {
                        parser_type: parser_value.to_string(),
                        field_name: None,
                        mode_type: None,
                        custom_fn: None,
                        convention: None,
                        data_variant: None,
                    };

                    // Continue parsing for additional parser attributes
                    attrs.parser = Some(parser_info);
                } else if part.contains("field") && attrs.parser.is_some() {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("field must have a value")
                        .trim()
                        .trim_matches('"');
                    if let Some(ref mut parser) = attrs.parser {
                        parser.field_name = Some(value.to_string());
                    }
                } else if (part.contains("type") || part.contains("value_type"))
                    && attrs.parser.is_some()
                {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("type/value_type must have a value")
                        .trim()
                        .trim_matches('"');
                    if let Some(ref mut parser) = attrs.parser {
                        parser.mode_type = Some(value.to_string());
                    }
                } else if (part.contains("custom_fn") || part.contains("parse_with"))
                    && attrs.parser.is_some()
                {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("custom_fn/parse_with must have a value")
                        .trim()
                        .trim_matches('"');
                    if let Some(ref mut parser) = attrs.parser {
                        parser.custom_fn = Some(value.to_string());
                    }
                } else if part.contains("convention") && attrs.parser.is_some() {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("convention must have a value")
                        .trim()
                        .trim_matches('"');
                    if let Some(ref mut parser) = attrs.parser {
                        parser.convention = Some(value.to_string());
                    }
                } else if part.contains("data_variant") && attrs.parser.is_some() {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("data_variant must have a value")
                        .trim()
                        .trim_matches('"');
                    if let Some(ref mut parser) = attrs.parser {
                        parser.data_variant = Some(value.to_string());
                    }
                } else if part.contains("constant") || part.contains("bytes_const") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("constant/bytes_const must have a value")
                        .trim()
                        .trim_matches('"');
                    attrs.constant = Some(value.to_string());
                }
            }
        }
    }

    attrs
}

fn generate_parser_body(
    response_variant: &Ident,
    parser_info: &ParserInfo,
    crate_path: &TokenStream,
) -> TokenStream {
    // Use data_variant if specified, otherwise use response_variant
    let actual_variant = parser_info
        .data_variant
        .as_deref()
        .map(|s| format_ident!("{}", s))
        .unwrap_or_else(|| response_variant.clone());

    match parser_info.parser_type.as_str() {
        "bool" => super::parser_templates::generate_bool_parser(response_variant, crate_path),
        "direct_byte" | "byte" => {
            let field_name = format_ident!("value"); // Default field name
            super::parser_templates::generate_direct_byte_parser(
                response_variant,
                &field_name,
                crate_path,
            )
        }
        "position" => {
            super::parser_templates::generate_position_parser(response_variant, crate_path)
        }
        "extended_nibble" | "nibble" => {
            let field_name = parser_info
                .field_name
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .unwrap_or_else(|| format_ident!("value"));
            super::parser_templates::generate_extended_nibble_parser(
                response_variant,
                &field_name,
                crate_path,
            )
        }
        "flags" | "bit_flags" => {
            super::parser_templates::generate_bit_flags_parser(response_variant, crate_path)
        }
        "mode" | "mode_enum" => {
            let mode_type = parser_info
                .mode_type
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .expect("mode parser requires type attribute");
            super::parser_templates::generate_mode_enum_parser(
                response_variant,
                &mode_type,
                crate_path,
            )
        }
        "pan_tilt" => {
            super::parser_templates::generate_pan_tilt_parser(response_variant, crate_path)
        }
        "bool_convention" => {
            let field_name = parser_info
                .field_name
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .expect("bool_convention parser requires field attribute");
            let convention = parser_info
                .convention
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .expect("bool_convention parser requires convention attribute (OnIs02 or OnIs03)");
            super::parser_templates::generate_bool_convention_parser(
                &actual_variant,
                &field_name,
                &convention,
                crate_path,
            )
        }
        "last_nibble" => {
            let field_name = parser_info
                .field_name
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .expect("last_nibble parser requires field attribute");
            super::parser_templates::generate_last_nibble_parser(
                &actual_variant,
                &field_name,
                crate_path,
            )
        }
        "tally_status" => super::parser_templates::generate_tally_status_parser(crate_path),
        "sharpness_mode" => super::parser_templates::generate_sharpness_mode_parser(crate_path),
        "gamma" => super::parser_templates::generate_gamma_parser(crate_path),
        "auto_wb_sensitivity" => {
            super::parser_templates::generate_auto_wb_sensitivity_parser(crate_path)
        }
        "nd_filter" => {
            let field_name = format_ident!("position");
            let converter_type = quote! { #crate_path::command::NdFilterPosition };
            super::parser_templates::generate_byte_converter_parser(
                &actual_variant,
                &field_name,
                &converter_type,
                super::parser_templates::ConverterMethod::FromByte,
                crate_path,
            )
        }
        "picture_effect" => {
            let field_name = format_ident!("effect");
            let converter_type = quote! { #crate_path::command::PictureEffectMode };
            super::parser_templates::generate_byte_converter_parser(
                &actual_variant,
                &field_name,
                &converter_type,
                super::parser_templates::ConverterMethod::FromByte,
                crate_path,
            )
        }
        "defog_level" => {
            let field_name = format_ident!("level");
            let converter_type = quote! { #crate_path::types::DefogLevel };
            super::parser_templates::generate_byte_converter_parser(
                &actual_variant,
                &field_name,
                &converter_type,
                super::parser_templates::ConverterMethod::New,
                crate_path,
            )
        }
        "focus_range" => {
            let field_name = format_ident!("range");
            let converter_type = quote! { #crate_path::command::FocusRange };
            super::parser_templates::generate_byte_converter_parser(
                &actual_variant,
                &field_name,
                &converter_type,
                super::parser_templates::ConverterMethod::TryFrom,
                crate_path,
            )
        }
        "custom" => {
            let custom_fn = parser_info
                .custom_fn
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .expect("custom parser requires custom_fn/parse_with attribute");
            quote! {
                #custom_fn(data)
            }
        }
        _ => {
            let parser_type_str = &parser_info.parser_type;
            quote! {
                return Err(#crate_path::Error::InvalidResponse {
                    expected: ::std::borrow::Cow::Owned(format!("Unknown parser type: {}", #parser_type_str)),
                    actual: data.to_vec(),
                })
            }
        }
    }
}

/// Generate an impl of `command::ResponseParser` for the struct based on
/// `typed_*` attributes. Returns empty tokens if no `typed_response` attribute is set.
fn generate_typed_impl(
    struct_name: &Ident,
    response_variant: &Ident,
    crate_path: &TokenStream,
    attrs: &ViscaAttributes,
) -> TokenStream {
    let Some(typed_response_str) = &attrs.typed_response else {
        return quote! {};
    };

    let response_type = parse_type_path(typed_response_str, crate_path);

    // Use data_variant from parser info if present, otherwise use response_variant
    let data_variant = attrs
        .parser
        .as_ref()
        .and_then(|p| p.data_variant.as_deref())
        .map(|s| format_ident!("{}", s))
        .unwrap_or_else(|| response_variant.clone());

    let Some(typed_field_str) = &attrs.typed_field else {
        return syn::Error::new_spanned(
            struct_name,
            "typed_response requires typed_field attribute",
        )
        .to_compile_error();
    };

    let field_names: Vec<Ident> = typed_field_str
        .split_whitespace()
        .map(|f| format_ident!("{}", f))
        .collect();

    // Generate the destructure pattern
    let destructure = if attrs.typed_is_tuple {
        quote! { #crate_path::command::InquiryData::#data_variant(#(#field_names),*) }
    } else {
        quote! { #crate_path::command::InquiryData::#data_variant { #(#field_names),* } }
    };

    // Generate the construction expression
    let construction = match attrs.typed_constructor.as_deref() {
        None => {
            let f = &field_names[0];
            quote! { Ok(#f) }
        }
        Some("new") => {
            let f = &field_names[0];
            quote! { #response_type::new(#f) }
        }
        Some("ok_new") => {
            let f = &field_names[0];
            quote! { Ok(#response_type::new(#f)) }
        }
        Some("new_u8") => {
            let f = &field_names[0];
            quote! { #response_type::new(#f as u8) }
        }
        Some("ok_struct") => {
            quote! { Ok(#response_type { #(#field_names),* }) }
        }
        Some(unknown) => {
            let msg = format!("unknown typed_constructor: {unknown}");
            return syn::Error::new_spanned(struct_name, msg).to_compile_error();
        }
    };

    quote! {
        impl #crate_path::command::ResponseParser for #struct_name {
            type Response = #response_type;

            fn from_response(
                resp: #crate_path::command::Response,
            ) -> Result<Self::Response, #crate_path::Error> {
                match resp {
                    #crate_path::command::Response::Inquiry(#destructure) => #construction,
                    #crate_path::command::Response::Error(e) => Err(e),
                    _ => Err(#crate_path::Error::UnexpectedResponseType),
                }
            }
        }
    }
}

/// Parse a type path string into a `TokenStream`.
///
/// Primitive types (`bool`, `u8`, `u16`, `i8`) are emitted directly.
/// All other paths are prefixed with the resolved crate path.
fn parse_type_path(type_str: &str, crate_path: &TokenStream) -> TokenStream {
    match type_str {
        "bool" => quote! { bool },
        "u8" => quote! { u8 },
        "u16" => quote! { u16 },
        "i8" => quote! { i8 },
        other => {
            let parts: Vec<Ident> = other
                .split("::")
                .map(|s| format_ident!("{}", s.trim()))
                .collect();
            quote! { #crate_path::#(#parts)::* }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downstream_inquiry_encoding_uses_stack_buffer_and_public_terminator() {
        let input: DeriveInput = syn::parse_quote! {
            #[visca(opcode = 0x47, response = "ZoomPosition")]
            struct CustomZoomInquiry;
        };

        let tokens = derive_visca_inquiry_impl(input).to_string();

        assert!(
            !tokens.contains("vec !"),
            "generated inquiry encoder must not allocate with vec!: {tokens}"
        );
        assert!(
            !tokens.contains("to_vec"),
            "generated inquiry encoder must not allocate with to_vec(): {tokens}"
        );
        assert!(
            !tokens.contains("0xFF"),
            "generated downstream inquiry encoder must use VISCA_TERMINATOR: {tokens}"
        );
        assert!(
            tokens.contains("command :: VISCA_TERMINATOR"),
            "generated downstream inquiry encoder must use the public terminator path: {tokens}"
        );
    }
}
