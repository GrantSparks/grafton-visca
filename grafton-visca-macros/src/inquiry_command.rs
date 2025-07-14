// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! InquiryCommand derive macro implementation with parser generation support

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident};

pub fn derive_inquiry_command_impl(input: DeriveInput) -> TokenStream {
    match &input.data {
        syn::Data::Struct(_) => {
            let struct_name = &input.ident;

            // Parse visca attributes from the struct
            let attrs = parse_visca_attributes_from_struct(&input);

            // Extract required attributes
            let byte_value = attrs
                .byte_value
                .expect("visca attribute must have a 'command' value");
            let response_type = attrs
                .response_type
                .expect("visca attribute must have a 'response' value");

            // Generate the bytes based on subcategory
            let bytes_expr = if let Some(sub) = attrs.subcategory {
                quote! { vec![0x81, 0x09, #sub, #byte_value, 0xFF] }
            } else {
                quote! { vec![0x81, 0x09, 0x04, #byte_value, 0xFF] }
            };

            // Determine crate path once for consistency
            let crate_path =
                if std::env::var("CARGO_PKG_NAME").unwrap_or_default() == "grafton-visca" {
                    quote! { crate }
                } else {
                    quote! { ::grafton_visca }
                };

            // Generate parser implementation if parser info is provided
            let parse_response_impl = if let Some(parser_info) = &attrs.parser {
                let parser_body = generate_parser_body(&response_type, parser_info, &crate_path);
                quote! {
                    impl #struct_name {
                        /// Parse the response data for this inquiry command
                        pub fn parse_response(&self, data: &[u8]) -> Result<#crate_path::command::InquiryResponse, #crate_path::Error> {
                            if data.is_empty() {
                                return Err(#crate_path::Error::InvalidResponseLength);
                            }
                            #parser_body
                        }
                    }
                }
            } else {
                quote! {}
            };

            let expanded = quote! {
                impl #crate_path::command::EncodeVisca for #struct_name {
                    type Response = #crate_path::command::InquiryResponse;
                    const MAX_SIZE: usize = 5;

                    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, #crate_path::Error> {
                        let bytes = #bytes_expr;
                        let len = bytes.len();
                        if buffer.len() < len {
                            return Err(#crate_path::Error::BufferTooSmall {
                                required: len,
                                actual: buffer.len(),
                            });
                        }
                        buffer[..len].copy_from_slice(&bytes);
                        Ok(len)
                    }

                    fn response_type(&self) -> Option<#crate_path::command::ResponseType> {
                        Some(#crate_path::command::ResponseType::#response_type)
                    }

                    fn timeout_kind(&self) -> #crate_path::timeout::CommandCategory {
                        #crate_path::timeout::CommandCategory::Quick
                    }
                }


                #parse_response_impl
            };

            expanded
        }
        _ => syn::Error::new_spanned(&input, "InquiryCommand can only be derived for structs")
            .to_compile_error(),
    }
}

#[derive(Default)]
struct ViscaAttributes {
    byte_value: Option<u8>,
    subcategory: Option<u8>,
    response_type: Option<Ident>,
    parser: Option<ParserInfo>,
}

struct ParserInfo {
    parser_type: String,
    field_name: Option<String>,
    offset: Option<i8>,
    mode_type: Option<String>,
    custom_fn: Option<String>,
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

                if part.contains("command") && !part.contains("sub_command") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("command must have a value")
                        .trim();
                    if value.starts_with("0x") {
                        attrs.byte_value = Some(
                            u8::from_str_radix(value.trim_start_matches("0x"), 16)
                                .expect("command must be a valid hex u8"),
                        );
                    } else {
                        attrs.byte_value =
                            Some(value.parse::<u8>().expect("command must be a valid u8"));
                    }
                } else if part.contains("response") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("response must have a value")
                        .trim()
                        .trim_matches('"');
                    // For struct attributes, response should be a simple string
                    attrs.response_type = Some(format_ident!("{}", value));
                } else if part.contains("sub_command") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("sub_command must have a value")
                        .trim();
                    if value.starts_with("0x") {
                        attrs.subcategory = Some(
                            u8::from_str_radix(value.trim_start_matches("0x"), 16)
                                .expect("sub_command must be a valid hex u8"),
                        );
                    } else {
                        attrs.subcategory =
                            Some(value.parse::<u8>().expect("sub_command must be a valid u8"));
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
                        offset: None,
                        mode_type: None,
                        custom_fn: None,
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
                } else if part.contains("offset") && attrs.parser.is_some() {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("offset must have a value")
                        .trim();
                    if let Some(ref mut parser) = attrs.parser {
                        parser.offset =
                            Some(value.parse::<i8>().expect("offset must be a valid i8"));
                    }
                } else if part.contains("type") && attrs.parser.is_some() {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("type must have a value")
                        .trim()
                        .trim_matches('"');
                    if let Some(ref mut parser) = attrs.parser {
                        parser.mode_type = Some(value.to_string());
                    }
                } else if part.contains("custom_fn") && attrs.parser.is_some() {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("custom_fn must have a value")
                        .trim()
                        .trim_matches('"');
                    if let Some(ref mut parser) = attrs.parser {
                        parser.custom_fn = Some(value.to_string());
                    }
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
        "offset" => {
            let field_name = parser_info
                .field_name
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .unwrap_or_else(|| format_ident!("value"));
            let offset = parser_info.offset.unwrap_or(0);
            super::parser_templates::generate_offset_parser(
                response_variant,
                &field_name,
                offset,
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
        "custom" => {
            let custom_fn = parser_info
                .custom_fn
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .expect("custom parser requires custom_fn attribute");
            quote! {
                #crate_path::command::response::#custom_fn(data)
            }
        }
        _ => {
            let parser_type_str = &parser_info.parser_type;
            quote! {
                return Err(#crate_path::Error::InvalidResponse {
                    expected: format!("Unknown parser type: {}", #parser_type_str),
                    actual: data.to_vec(),
                })
            }
        }
    }
}
