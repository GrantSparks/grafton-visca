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
                .expect("visca attribute must have a 'response' value");

            let constant_name = attrs
                .constant
                .expect("visca attribute must have a 'constant' or 'bytes_const' value - all inquiries must use predefined constants");

            // Determine crate path once for consistency
            let crate_path =
                if std::env::var("CARGO_PKG_NAME").unwrap_or_default() == "grafton-visca" {
                    quote! { crate }
                } else {
                    quote! { ::grafton_visca }
                };

            // Use the predefined constant
            let constant_path = format_ident!("{}", constant_name);
            let bytes_expr = quote! {
                {
                    let mut bytes = #crate_path::command::bytes::constants::inquiry::#constant_path.to_vec();
                    // Replace camera ID (first byte)
                    bytes[0] = camera_id.to_address_byte();
                    bytes
                }
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

            // Optionally generate a typed ViscaCommand impl for a subset of inquiries
            let typed_impl = generate_typed_impl(
                struct_name,
                &response_kind,
                attrs.parser.as_ref(),
                &crate_path,
            );

            let expanded = quote! {
                impl #crate_path::command::ViscaCommand for #struct_name {
                    type Response = #crate_path::command::InquiryData;
                    const MAX_SIZE: usize = 5;
                    const TIMEOUT_CATEGORY: #crate_path::timeout::CommandCategory = #crate_path::timeout::CommandCategory::Quick;

                    fn write_into(&self, camera_id: #crate_path::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, #crate_path::Error> {
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
    constant: Option<String>, // Name of the constant to use
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

                if (part.contains("command") && !part.contains("sub_command"))
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
                .expect("custom parser requires custom_fn/parse_with attribute");
            quote! {
                #crate_path::command::response::#custom_fn(data)
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

/// Optionally generate an impl of `command::typed::ResponseParser` for the struct
/// when we can map its `response` to a clear concrete type.
fn generate_typed_impl(
    struct_name: &Ident,
    response_variant: &Ident,
    parser_info: Option<&ParserInfo>,
    crate_path: &TokenStream,
) -> TokenStream {
    // We currently support three common inquiry types for typed responses:
    // - Power (bool)
    // - PanTiltPosition (camera::PanTiltPosition)
    // - ZoomPosition (u16)
    let var = response_variant.to_string();
    match (
        var.as_str(),
        parser_info.map(|p| p.parser_type.as_str()),
        parser_info.and_then(|p| p.mode_type.as_deref()),
    ) {
        ("Power", Some("bool"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = bool;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Power { on }) => Ok(on),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("PanTiltPosition", Some("pan_tilt"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::camera::PanTiltPosition;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::PanTiltPosition { pan, tilt }
                            ) => Ok(#crate_path::camera::PanTiltPosition { pan, tilt }),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("ZoomPosition", Some("position"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::ZoomPosition;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::ZoomPosition { position }
                            ) => #crate_path::types::ZoomPosition::new(position),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("FocusNearLimit", Some("position"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::FocusPosition;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::FocusNearLimit { position }
                            ) => #crate_path::types::FocusPosition::new(position),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("FocusPosition", Some("position"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::FocusPosition;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::FocusPosition { position }
                            ) => #crate_path::types::FocusPosition::new(position),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("ExposureMode", Some("mode"), Some("ExposureMode")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::ExposureMode;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::ExposureMode { mode }
                            ) => Ok(mode),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("FocusMode", Some("mode"), Some("FocusMode")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::FocusMode;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::FocusMode { mode }
                            ) => Ok(mode),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("WhiteBalanceMode", Some("mode"), Some("WhiteBalanceMode")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::WhiteBalanceMode;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::WhiteBalanceMode { mode }
                            ) => Ok(mode),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("BlackWhiteMode", Some("mode"), Some("BlackWhiteMode")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::BlackWhiteMode;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::BlackWhiteMode { mode }
                            ) => Ok(mode),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("MotionSyncMode", Some("mode"), Some("MotionSyncMode")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::MotionSyncMode;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::MotionSyncMode { mode }
                            ) => Ok(mode),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("MotionSyncSpeed", Some("mode"), Some("MotionSyncSpeed")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::MotionSyncSpeed;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::MotionSyncSpeed { speed }
                            ) => Ok(speed),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("MotionSyncPreset", Some("speed"), Some("MotionSyncPreset")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::MotionSyncPreset;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::MotionSyncPreset { speed }
                            ) => Ok(speed),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("NrMode", Some("mode"), Some("NrMode")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::NrMode;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::NrMode { mode }
                            ) => Ok(mode),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("NrSpeed", Some("mode"), Some("NrSpeed")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::NrSpeed;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::NrSpeed { speed }
                            ) => Ok(speed),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("NrSpeed", Some("speed"), Some("NrSpeed")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::NrSpeed;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::NrSpeed { speed }
                            ) => Ok(speed),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("FocusZone", Some("mode"), Some("FocusZone")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::FocusZone;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::FocusZone { zone }
                            ) => Ok(zone),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("AutoWhiteBalanceSensitivity", _, Some("AutoWhiteBalanceSensitivity")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::AutoWhiteBalanceSensitivity;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::AutoWhiteBalanceSensitivity { sensitivity }
                            ) => Ok(sensitivity),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("FocusRange", _, Some("FocusRange")) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::FocusRange;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::FocusRange { range }
                            ) => Ok(range),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("GainLimit", Some("byte"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::GainLimit;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::GainLimit { limit }
                            ) => #crate_path::types::GainLimit::new(limit),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("NrLevel", Some("byte"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = u8;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::NrLevel(val)) => Ok(val),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("Resolution", Some("byte"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::command::resolution::ResolutionMode;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Resolution(val)) => Ok(val),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("BroadcastDomain", Some("byte"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::BroadcastDomain;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::BroadcastDomain(val)) => Ok(val),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("NoiseReduction2D", Some("byte"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::NoiseReduction2DLevel;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::NoiseReduction2D { level }) => #crate_path::types::NoiseReduction2DLevel::new(level),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("NoiseReduction3D", Some("byte"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::NoiseReduction3DLevel;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::NoiseReduction3D { level }) => #crate_path::types::NoiseReduction3DLevel::new(level),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("Gamma", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::GammaLevel;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Gamma { value }) => #crate_path::types::GammaLevel::new(value),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("RedTuning", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = u8;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::RedTuning { level }) => Ok(level),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("BlueTuning", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = u8;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::BlueTuning { level }) => Ok(level),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("DefogLevel", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::DefogLevel;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::DefogLevel { level }) => Ok(level),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("ExposureCompensationPosition", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = #crate_path::types::ExposureCompensationPosition;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::ExposureCompensationPosition { position }
                            ) => Ok(position),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("Standby", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = bool;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Standby { in_standby }) => Ok(in_standby),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("DigitalPtz", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = bool;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::DigitalPtz { enabled }) => Ok(enabled),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("AutoTrace", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = bool;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::AutoTrace { enabled }) => Ok(enabled),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("FocusUnlock", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = bool;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::FocusUnlock { unlocked }) => Ok(unlocked),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("IrisControl", _, _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = bool;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::IrisControl { auto }) => Ok(auto),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("Backlight", Some("bool"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = bool;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::Backlight { status }
                            ) => Ok(status),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("BlackWhite", Some("bool"), _) => {
            quote! {
                impl #crate_path::command::typed::ResponseParser for #struct_name {
                    type Response = bool;

                    fn from_response(resp: #crate_path::command::Response) -> Result<Self::Response, #crate_path::Error> {
                        match resp {
                            #crate_path::command::Response::Inquiry(
                                #crate_path::command::InquiryData::BlackWhite { on }
                            ) => Ok(on),
                            #crate_path::command::Response::Error(e) => Err(e),
                            _ => Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            }
        }
        ("ExposureCompensationMode", Some("bool"), _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = bool; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::ExposureCompensationMode{ on })=>Ok(on), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("ExposureCompensation", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = i8; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::ExposureCompensation{ value })=>Ok(value), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Bright", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = u16; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Bright{ position })=>Ok(position), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Brightness", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::types::BrightnessLevel; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Brightness{ position })=>#crate_path::types::BrightnessLevel::new(position), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Iris", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::types::IrisLevel; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Iris{ position })=>#crate_path::types::IrisLevel::new(position), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Shutter", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = u16; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Shutter{ position })=>Ok(position), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("ColorTemperature", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = u16; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::ColorTemperature{ temperature })=>Ok(temperature), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("RedChannel", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::types::RedChannel; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::RedChannel{ gain })=>#crate_path::types::RedChannel::new(gain as u8), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("BlueChannel", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::types::BlueChannel; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::BlueChannel{ gain })=>#crate_path::types::BlueChannel::new(gain as u8), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Saturation", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::types::SaturationLevel; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Saturation{ level })=>#crate_path::types::SaturationLevel::new(level), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Hue", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::types::HueLevel; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Hue{ hue })=>#crate_path::types::HueLevel::new(hue), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Gain", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::types::GainLevel; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::GainLevel{ gain })=>#crate_path::types::GainLevel::new(gain), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("SharpnessMode", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::SharpnessMode; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::SharpnessMode{ mode })=>Ok(mode), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("ImageFlip", Some("flags"), _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::typed::FlipState; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::ImageFlip{ vertical, horizontal })=>Ok(#crate_path::command::typed::FlipState{ horizontal, vertical }), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("FlipMode", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::typed::FlipState; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::FlipMode{ horizontal, vertical })=>Ok(#crate_path::command::typed::FlipState{ horizontal, vertical }), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("FlipState", Some("flags"), _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::typed::FlipState; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::FlipState{ vertical, horizontal })=>Ok(#crate_path::command::typed::FlipState{ horizontal, vertical }), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("FlipState", Some("custom"), _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::typed::FlipState; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::FlipState{ horizontal, vertical })=>Ok(#crate_path::command::typed::FlipState{ horizontal, vertical }), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("NoiseReductionLevel", Some("byte"), _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::types::NoiseReductionLevel; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::NoiseReductionLevel(val))=>#crate_path::types::NoiseReductionLevel::new(val), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("NoiseReductionMode", Some("mode"), Some("NoiseReductionMode")) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::NoiseReductionMode; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::NoiseReductionMode{ mode })=>Ok(mode), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Version", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::typed::VersionInfo; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Version{ vendor, model, rom_version, max_socket })=>Ok(#crate_path::command::typed::VersionInfo{ vendor, model, rom_version, max_socket }), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("TallyStatus", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::typed::TallyStatusState; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::TallyStatus{ red_on, green_on })=>Ok(#crate_path::command::typed::TallyStatusState{ red_on, green_on }), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("UsbAudio", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = bool; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::UsbAudio{ on })=>Ok(on), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("TwoToneMode", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = bool; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::TwoToneMode{ on })=>Ok(on), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Digital", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = bool; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Digital{ on })=>Ok(on), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("TallyAutoAdjust", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = bool; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::TallyAutoAdjust{ on })=>Ok(on), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("Rtmp", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = bool; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::Rtmp{ on })=>Ok(on), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("MenuOpenClose", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = bool; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::MenuOpenClose{ is_open })=>Ok(is_open), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("AutoFocus", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = bool; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::AutoFocus{ enabled })=>Ok(enabled), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("NightDayMode", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = bool; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::NightDayMode{ is_night })=>Ok(is_night), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("NdFilterPreset", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::types::NdFilterPreset; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::NdFilterPreset{ preset })=>Ok(preset), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("NdFilter", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::resolution::NdFilterPosition; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::NdFilter{ position })=>Ok(position), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        ("PictureEffect", _, _) => {
            quote! { impl #crate_path::command::typed::ResponseParser for #struct_name { type Response = #crate_path::command::resolution::PictureEffectMode; fn from_response(resp:#crate_path::command::Response)->Result<Self::Response,#crate_path::Error>{ match resp { #crate_path::command::Response::Inquiry(#crate_path::command::InquiryData::PictureEffect{ effect })=>Ok(effect), #crate_path::command::Response::Error(e)=>Err(e), _=>Err(#crate_path::Error::UnexpectedResponseType), } } } }
        }
        _ => quote! {},
    }
}
