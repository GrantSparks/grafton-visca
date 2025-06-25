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
            let inquiry_variant = attrs
                .inquiry_variant
                .expect("visca attribute must have an 'inquiry_variant' value");

            // Generate to_bytes implementation
            let to_bytes_impl = if let Some(sub) = attrs.subcategory {
                quote! {
                    vec![0x81, 0x09, #sub, #byte_value, 0xFF]
                }
            } else {
                quote! {
                    vec![0x81, 0x09, 0x04, #byte_value, 0xFF]
                }
            };

            // Generate parser implementation if specified
            let parse_response_impl = if let Some(parser_info) = &attrs.parser {
                let response_variant_for_parser = attrs.response_variant.as_ref().unwrap_or(&response_type);
                let parser_body = generate_parser_body(response_variant_for_parser, parser_info);
                quote! {
                    /// Parse the response data for this inquiry command
                    pub fn parse_response(&self, data: &[u8]) -> Result<crate::command::InquiryResponse, crate::Error> {
                        if data.is_empty() {
                            return Err(crate::Error::InvalidResponseLength);
                        }
                        Ok(#parser_body)
                    }
                }
            } else {
                quote! {}
            };

            // Generate the Command trait implementation
            let expanded = quote! {
                impl crate::command::Command for #struct_name {
                    fn to_bytes(&self) -> Result<Vec<u8>, crate::Error> {
                        Ok(#to_bytes_impl)
                    }

                    fn response_type(&self) -> Option<crate::command::ResponseType> {
                        Some(crate::command::ResponseType::#response_type)
                    }

                    fn command_category(&self) -> crate::timeout::CommandCategory {
                        crate::timeout::CommandCategory::Quick
                    }
                }

                impl #struct_name {
                    #parse_response_impl
                }

                // Generate From conversion for InquiryCommand enum
                impl From<#struct_name> for crate::command::InquiryCommand {
                    fn from(_: #struct_name) -> Self {
                        crate::command::InquiryCommand::#inquiry_variant
                    }
                }
            };

            expanded
        }
        syn::Data::Enum(enum_data) => {
            let enum_name = &input.ident;
            let variants = &enum_data.variants;

            // Generate to_bytes match arms
            let to_bytes_arms = variants.iter().map(|variant| {
                let variant_name = &variant.ident;

                // Parse visca attributes
                let attrs = parse_visca_attributes(variant);
                let byte_value = attrs
                    .byte_value
                    .expect("visca attribute must have a byte value");

                if let Some(sub) = attrs.subcategory {
                    quote! {
                        Self::#variant_name => vec![0x81, 0x09, #sub, #byte_value, 0xFF],
                    }
                } else {
                    quote! {
                        Self::#variant_name => vec![0x81, 0x09, 0x04, #byte_value, 0xFF],
                    }
                }
            });

            // Generate response_type match arms
            let response_type_arms = variants.iter().map(|variant| {
                let variant_name = &variant.ident;
                let attrs = parse_visca_attributes(variant);
                let response_type = attrs.response_type.expect("visca attribute must have a response type");
                quote! {
                    Self::#variant_name => Some(crate::command::ResponseType::#response_type),
                }
            });

            // Generate parser match arms
            let parser_arms = variants.iter().filter_map(|variant| {
                let variant_name = &variant.ident;
                let attrs = parse_visca_attributes(variant);

                // Only generate parser if parser attribute is present
                attrs.parser.as_ref().map(|parser_info| {
                    let response_variant = &attrs.response_type.expect("response type required");
                    generate_parser_arm(variant_name, response_variant, parser_info)
                })
            });

            // Generate the parse_response method if any parsers are defined
            let parse_response_impl = if parser_arms.clone().count() > 0 {
                quote! {
                    /// Parse the response data for this inquiry command
                    pub fn parse_response(&self, data: &[u8]) -> Result<crate::command::InquiryResponse, crate::Error> {
                        if data.is_empty() {
                            return Err(crate::Error::InvalidResponseLength);
                        }

                        match self {
                            #(#parser_arms)*
                            _ => Err(crate::Error::InvalidResponse {
                                expected: "Parser not implemented for this command".to_string(),
                                actual: data.to_vec(),
                            }),
                        }
                    }
                }
            } else {
                quote! {}
            };

            let expanded = quote! {
                impl crate::command::Command for #enum_name {
                    fn to_bytes(&self) -> Result<Vec<u8>, crate::Error> {
                        let bytes = match self {
                            #(#to_bytes_arms)*
                        };
                        Ok(bytes)
                    }

                    fn response_type(&self) -> Option<crate::command::ResponseType> {
                        match self {
                            #(#response_type_arms)*
                        }
                    }

                    fn command_category(&self) -> crate::timeout::CommandCategory {
                        crate::timeout::CommandCategory::Quick
                    }
                }

                impl #enum_name {
                    #parse_response_impl
                }
            };

            expanded
        }
        _ => syn::Error::new_spanned(&input, "InquiryCommand can only be derived for enums")
            .to_compile_error(),
    }
}

#[derive(Default)]
struct ViscaAttributes {
    byte_value: Option<u8>,
    subcategory: Option<u8>,
    response_type: Option<Ident>,
    response_variant: Option<Ident>,
    parser: Option<ParserInfo>,
    inquiry_variant: Option<Ident>,
}

struct ParserInfo {
    parser_type: String,
    field_name: Option<String>,
    offset: Option<i8>,
    mode_type: Option<String>,
    custom_fn: Option<String>,
}

fn parse_visca_attributes(variant: &syn::Variant) -> ViscaAttributes {
    let mut attrs = ViscaAttributes::default();

    for attr in &variant.attrs {
        if attr.path().is_ident("visca") {
            // Parse the attribute manually
            let tokens = attr
                .parse_args::<proc_macro2::TokenStream>()
                .expect("Failed to parse visca attribute");
            let token_str = tokens.to_string();

            // Split by comma and parse each part
            for part in token_str.split(',') {
                let part = part.trim();

                if part.contains("response") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("response must have a value")
                        .trim();
                    // Extract just the variant name, not the full type definition
                    let variant_name = if value.contains('{') {
                        value.split('{').next().unwrap().trim()
                    } else if value.contains('(') {
                        value.split('(').next().unwrap().trim()
                    } else {
                        value
                    };
                    attrs.response_type = Some(format_ident!("{}", variant_name));
                } else if part.contains("subcategory") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("subcategory must have a value")
                        .trim();
                    if value.starts_with("0x") {
                        attrs.subcategory = Some(
                            u8::from_str_radix(value.trim_start_matches("0x"), 16)
                                .expect("subcategory must be a valid hex u8"),
                        );
                    } else {
                        attrs.subcategory =
                            Some(value.parse::<u8>().expect("subcategory must be a valid u8"));
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
                } else if part.starts_with("0x") || part.chars().all(|c| c.is_ascii_hexdigit()) {
                    // This is the hex byte value
                    attrs.byte_value = Some(
                        u8::from_str_radix(part.trim_start_matches("0x"), 16)
                            .expect("Invalid hex value"),
                    );
                }
            }
        }
    }

    attrs
}

fn generate_parser_arm(
    variant_name: &Ident,
    response_variant: &Ident,
    parser_info: &ParserInfo,
) -> TokenStream {
    let parser_body = match parser_info.parser_type.as_str() {
        "bool" => {
            let field_name = parser_info.field_name.as_ref()
                .map(|f| format_ident!("{}", f));
            super::parser_templates::generate_bool_parser(response_variant, field_name.as_ref())
        }
        "direct_byte" | "byte" => {
            let field_name = parser_info.field_name.as_ref()
                .map(|f| format_ident!("{}", f));
            super::parser_templates::generate_direct_byte_parser(response_variant, field_name.as_ref())
        }
        "position" => super::parser_templates::generate_position_parser(response_variant),
        "extended_nibble" | "nibble" => {
            let field_name = parser_info
                .field_name
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .unwrap_or_else(|| format_ident!("value"));
            super::parser_templates::generate_extended_nibble_parser(response_variant, &field_name)
        }
        "offset" => {
            let field_name = parser_info
                .field_name
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .unwrap_or_else(|| format_ident!("value"));
            let offset = parser_info.offset.unwrap_or(0);
            super::parser_templates::generate_offset_parser(response_variant, &field_name, offset)
        }
        "flags" | "bit_flags" => {
            super::parser_templates::generate_bit_flags_parser(response_variant)
        }
        "mode" | "mode_enum" => {
            let mode_type = parser_info
                .mode_type
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .expect("mode parser requires type attribute");
            let field_name = parser_info.field_name.as_ref()
                .map(|f| format_ident!("{}", f));
            super::parser_templates::generate_mode_enum_parser(response_variant, &mode_type, field_name.as_ref())
        }
        "pan_tilt" => super::parser_templates::generate_pan_tilt_parser(response_variant),
        "custom" => {
            let custom_fn = parser_info
                .custom_fn
                .as_deref()
                .map(|s| format_ident!("{}", s))
                .expect("custom parser requires custom_fn attribute");
            quote! {
                #custom_fn(data)?
            }
        }
        _ => {
            let parser_type_str = &parser_info.parser_type;
            quote! {
                return Err(crate::Error::InvalidResponse {
                    expected: format!("Unknown parser type: {}", #parser_type_str),
                    actual: data.to_vec(),
                })
            }
        }
    };

    quote! {
        Self::#variant_name => Ok(#parser_body),
    }
}

/// Helper function to generate parser body
fn generate_parser_body(response_variant: &Ident, parser_info: &ParserInfo) -> TokenStream {
    match parser_info.parser_type.as_str() {
        "bool" => {
            let field_name = parser_info.field_name.as_ref()
                .map(|f| format_ident!("{}", f));
            super::parser_templates::generate_bool_parser(response_variant, field_name.as_ref())
        }
        "direct_byte" | "byte" => {
            let field_name = parser_info.field_name.as_ref()
                .map(|f| format_ident!("{}", f));
            super::parser_templates::generate_direct_byte_parser(response_variant, field_name.as_ref())
        }
        "position" => super::parser_templates::generate_position_parser(response_variant),
        "extended_nibble" | "nibble" => {
            let field_name = parser_info.field_name.as_ref()
                .map(|f| format_ident!("{}", f))
                .unwrap_or_else(|| format_ident!("value"));
            super::parser_templates::generate_extended_nibble_parser(response_variant, &field_name)
        }
        "offset" => {
            let field_name = parser_info.field_name.as_ref()
                .map(|f| format_ident!("{}", f))
                .unwrap_or_else(|| format_ident!("value"));
            let offset = parser_info.offset.unwrap_or(0);
            super::parser_templates::generate_offset_parser(response_variant, &field_name, offset)
        }
        "flags" => super::parser_templates::generate_bit_flags_parser(response_variant),
        "mode" => {
            let mode_type = parser_info.mode_type.as_ref()
                .map(|t| format_ident!("{}", t))
                .expect("mode parser requires mode_type");
            let field_name = parser_info.field_name.as_ref()
                .map(|f| format_ident!("{}", f));
            super::parser_templates::generate_mode_enum_parser(response_variant, &mode_type, field_name.as_ref())
        }
        "pan_tilt" => super::parser_templates::generate_pan_tilt_parser(response_variant),
        _ => {
            let parser_type_str = &parser_info.parser_type;
            quote! {
                return Err(crate::Error::InvalidResponse {
                    expected: format!("Unknown parser type: {}", #parser_type_str),
                    actual: data.to_vec(),
                })
            }
        }
    }
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
            // Replace newlines with spaces to handle multi-line attributes
            let token_str = token_str.replace('\n', " ").replace('\r', " ");
            for part in token_str.split(',') {
                let part = part.trim();

                if part.contains("sub_command") || part.contains("subcategory") {
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
                } else if part.contains("command") && !part.contains("sub_") {
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
                        attrs.byte_value = Some(value.parse::<u8>().expect("command must be a valid u8"));
                    }
                } else if part.contains("inquiry_variant") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("inquiry_variant must have a value")
                        .trim()
                        .trim_matches('"');
                    attrs.inquiry_variant = Some(format_ident!("{}", value));
                } else if part.contains("response_variant") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("response_variant must have a value")
                        .trim()
                        .trim_matches('"');
                    attrs.response_variant = Some(format_ident!("{}", value));
                } else if part.contains("response") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("response must have a value")
                        .trim()
                        .trim_matches('"');
                    attrs.response_type = Some(format_ident!("{}", value));
                } else if part.contains("parser") {
                    let parser_value = part
                        .split('=')
                        .nth(1)
                        .expect("parser must have a value")
                        .trim()
                        .trim_matches('"');
                    attrs.parser = Some(ParserInfo {
                        parser_type: parser_value.to_string(),
                        field_name: None,
                        offset: None,
                        mode_type: None,
                        custom_fn: None,
                    });
                } else if part.contains("field") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("field must have a value")
                        .trim()
                        .trim_matches('"');
                    if let Some(ref mut parser) = attrs.parser {
                        parser.field_name = Some(value.to_string());
                    }
                } else if part.contains("offset") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("offset must have a value")
                        .trim();
                    if let Some(ref mut parser) = attrs.parser {
                        parser.offset = Some(value.parse::<i8>().expect("offset must be a valid i8"));
                    }
                } else if part.contains("mode_type") {
                    let value = part
                        .split('=')
                        .nth(1)
                        .expect("mode_type must have a value")
                        .trim()
                        .trim_matches('"');
                    if let Some(ref mut parser) = attrs.parser {
                        parser.mode_type = Some(value.to_string());
                    }
                }
            }
        }
    }

    attrs
}
