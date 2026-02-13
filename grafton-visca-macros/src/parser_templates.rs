// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Parser template generators for VISCA response parsing
//!
//! This module provides template generators for the 8 common parser patterns
//! identified in the VISCA protocol responses.
//!
//! # Parser Categories
//!
//! Based on analysis of the VISCA protocol, response parsers fall into these categories:
//!
//! 1. **Boolean**: Power, Backlight (0x02 = on/true, 0x03 = off/false)
//! 2. **Direct byte**: Luminance, Contrast, Saturation, Hue (raw byte value)
//! 3. **Position (4-nibble)**: ZoomPosition, FocusPosition (4 nibbles -> u16)
//! 4. **Extended nibble**: Sharpness, Shutter, ColorTemperature (2 nibbles -> u8)
//! 5. **Offset values**: RedGain, BlueGain (subtract offset), ExposureCompensation
//! 6. **Bit flags**: ImageFlip (bit 0 = horizontal, bit 1 = vertical)
//! 7. **Mode enums**: ExposureMode, WhiteBalanceMode (enum from byte)
//! 8. **Special**: PanTiltPosition (two signed 16-bit values)
//!
//! # Usage
//!
//! These generators are used by the ViscaInquiry derive macro to automatically
//! create parser functions based on the `parser` attribute value.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

/// Generate a boolean parser (0x02 = on/true, 0x03 = off/false)
pub fn generate_bool_parser(response_variant: &Ident, crate_path: &TokenStream) -> TokenStream {
    // Handle different field names for boolean responses
    let field_name = match response_variant.to_string().as_str() {
        "Backlight" => quote! { status },
        _ => quote! { on },
    };

    quote! {
        {
            match data[0] {
                0x02 => Ok(#crate_path::command::InquiryData::#response_variant { #field_name: true }),
                0x03 => Ok(#crate_path::command::InquiryData::#response_variant { #field_name: false }),
                _ => Err(#crate_path::Error::InvalidResponse {
                    expected: ::std::borrow::Cow::Borrowed("0x02 (on) or 0x03 (off)"),
                    actual: vec![data[0]],
                }),
            }
        }
    }
}

/// Generate a direct byte parser (value is used as-is)
pub fn generate_direct_byte_parser(
    response_variant: &Ident,
    _field_name: &Ident,
    crate_path: &TokenStream,
) -> TokenStream {
    match response_variant.to_string().as_str() {
        "GainLimit" => {
            quote! {
                Ok(#crate_path::command::InquiryData::#response_variant { limit: data[0] })
            }
        }
        "NoiseReduction2D" | "NoiseReduction3D" | "DynamicRange" => {
            quote! {
                Ok(#crate_path::command::InquiryData::#response_variant { level: data[0] })
            }
        }
        "NdFilterPreset" => {
            quote! {
                {
                    let preset = #crate_path::types::NdFilterPreset::new(data[0])
                        .map_err(|_| #crate_path::Error::InvalidParameter {
                            parameter: "nd_filter_preset",
                            value: ::std::borrow::Cow::Owned(data[0].to_string()),
                            reason: ::std::borrow::Cow::Borrowed("value out of range (0-3)"),
                        })?;
                    Ok(#crate_path::command::InquiryData::#response_variant { preset })
                }
            }
        }
        "DefogLevel" => {
            quote! {
                {
                    let level = #crate_path::types::DefogLevel::new(data[0])
                        .map_err(|_| #crate_path::Error::InvalidParameter {
                            parameter: "defog_level",
                            value: ::std::borrow::Cow::Owned(data[0].to_string()),
                            reason: ::std::borrow::Cow::Borrowed("value out of range (0-5)"),
                        })?;
                    Ok(#crate_path::command::InquiryData::#response_variant { level })
                }
            }
        }
        "BroadcastDomain" => {
            quote! {
                {
                    let domain = #crate_path::types::BroadcastDomain::new(data[0])
                        .map_err(|_| #crate_path::Error::InvalidParameter {
                            parameter: "broadcast_domain",
                            value: ::std::borrow::Cow::Owned(data[0].to_string()),
                            reason: ::std::borrow::Cow::Borrowed("value out of range (0-3)"),
                        })?;
                    Ok(#crate_path::command::InquiryData::#response_variant(domain))
                }
            }
        }
        "Resolution" => {
            quote! {
                {
                    let mode = #crate_path::command::resolution::ResolutionMode::from_byte(data[0]);
                    Ok(#crate_path::command::InquiryData::#response_variant(mode))
                }
            }
        }
        _ => {
            // Default to tuple variant
            quote! {
                Ok(#crate_path::command::InquiryData::#response_variant(data[0]))
            }
        }
    }
}

/// Generate a position parser (4 nibbles combined into u16)
/// Format: [0x0p, 0x0q, 0x0r, 0x0s] -> 0xpqrs
pub fn generate_position_parser(response_variant: &Ident, crate_path: &TokenStream) -> TokenStream {
    quote! {
        {
            if data.len() < 4 {
                return Err(#crate_path::Error::invalid_response_length(4, data));
            }
            let position = ((data[0] & 0x0F) as u16) << 12
                | ((data[1] & 0x0F) as u16) << 8
                | ((data[2] & 0x0F) as u16) << 4
                | (data[3] & 0x0F) as u16;
            Ok(#crate_path::command::InquiryData::#response_variant { position })
        }
    }
}

/// Generate an extended nibble parser (2 nibbles combined)
/// Format: [0x0p, 0x0q] -> value
pub fn generate_extended_nibble_parser(
    response_variant: &Ident,
    field_name: &Ident,
    crate_path: &TokenStream,
) -> TokenStream {
    quote! {
        {
            if data.len() < 2 {
                return Err(#crate_path::Error::invalid_response_length(2, data));
            }
            let value = ((data[0] & 0x0F) << 4) | (data[1] & 0x0F);
            Ok(#crate_path::command::InquiryData::#response_variant {
                #field_name: value as u8,
            })
        }
    }
}

/// Generate an offset value parser (subtracts offset from byte value)
pub fn generate_offset_parser(
    response_variant: &Ident,
    field_name: &Ident,
    offset: i8,
    crate_path: &TokenStream,
) -> TokenStream {
    quote! {
        {
            Ok(#crate_path::command::InquiryData::#response_variant {
                #field_name: (data[0] as i8) - #offset,
            })
        }
    }
}

/// Generate a bit flags parser
pub fn generate_bit_flags_parser(
    response_variant: &Ident,
    crate_path: &TokenStream,
) -> TokenStream {
    quote! {
        {
            Ok(#crate_path::command::InquiryData::#response_variant {
                horizontal: (data[0] & 0x01) != 0,
                vertical: (data[0] & 0x02) != 0,
            })
        }
    }
}

/// Generate a mode enum parser
pub fn generate_mode_enum_parser(
    response_variant: &Ident,
    mode_type: &Ident,
    crate_path: &TokenStream,
) -> TokenStream {
    // Determine the field name based on the response variant
    let field_name = match response_variant.to_string().as_str() {
        "FocusZone" => quote! { zone },
        "AutoFocusSensitivity" => quote! { sensitivity },
        _ => quote! { mode },
    };

    quote! {
        {
            let value = <#crate_path::command::#mode_type as TryFrom<u8>>::try_from(data[0])
                .map_err(|_| #crate_path::Error::InvalidResponse {
                    expected: ::std::borrow::Cow::Owned(format!("Valid {} value", stringify!(#mode_type))),
                    actual: vec![data[0]],
                })?;
            Ok(#crate_path::command::InquiryData::#response_variant { #field_name: value })
        }
    }
}

/// Generate a special parser for PanTiltPosition (two signed 16-bit values)
pub fn generate_pan_tilt_parser(response_variant: &Ident, crate_path: &TokenStream) -> TokenStream {
    quote! {
        {
            if data.len() < 8 {
                return Err(#crate_path::Error::invalid_response_length(8, data));
            }

            // Pan position (bytes 0-3)
            let pan = ((data[0] & 0x0F) as u16) << 12
                | ((data[1] & 0x0F) as u16) << 8
                | ((data[2] & 0x0F) as u16) << 4
                | (data[3] & 0x0F) as u16;
            let pan = pan as i16;

            // Tilt position (bytes 4-7)
            let tilt = ((data[4] & 0x0F) as u16) << 12
                | ((data[5] & 0x0F) as u16) << 8
                | ((data[6] & 0x0F) as u16) << 4
                | (data[7] & 0x0F) as u16;
            let tilt = tilt as i16;

            Ok(#crate_path::command::InquiryData::#response_variant { pan, tilt })
        }
    }
}

/// Generate a boolean parser using BoolConvention
///
/// Uses `Payload::parse_bool()` with explicit convention for type-safe parsing.
pub fn generate_bool_convention_parser(
    response_variant: &Ident,
    field_name: &Ident,
    convention: &Ident,
    crate_path: &TokenStream,
) -> TokenStream {
    let param_name = field_name.to_string();
    quote! {
        {
            let payload = #crate_path::command::response::Payload::new(data);
            let #field_name = payload.parse_bool(
                #param_name,
                #crate_path::command::response::BoolConvention::#convention
            )?;
            Ok(#crate_path::command::InquiryData::#response_variant { #field_name })
        }
    }
}

/// Generate a last-nibble parser (extracts last nibble from 4-byte payload)
///
/// Uses `Nibbles::<4>::try_from()` for type-safe nibble extraction.
pub fn generate_last_nibble_parser(
    response_variant: &Ident,
    field_name: &Ident,
    crate_path: &TokenStream,
) -> TokenStream {
    quote! {
        {
            let payload = #crate_path::command::response::Payload::new(data);
            let nibbles = #crate_path::command::response::payload::Nibbles::<4>::try_from(payload)?;
            Ok(#crate_path::command::InquiryData::#response_variant {
                #field_name: nibbles.last_nibble()
            })
        }
    }
}

/// Generate a tally status parser (2-byte boolean response)
///
/// Parses red (byte 0) and green (byte 1) tally states using BoolConvention.
pub fn generate_tally_status_parser(crate_path: &TokenStream) -> TokenStream {
    quote! {
        {
            if data.len() < 2 {
                return Err(#crate_path::Error::invalid_response_length(2, data));
            }
            let red_payload = #crate_path::command::response::Payload::new(&data[0..1]);
            let green_payload = #crate_path::command::response::Payload::new(&data[1..2]);
            let red_on = red_payload.parse_bool(
                "tally_red_status",
                #crate_path::command::response::BoolConvention::OnIs03
            )?;
            let green_on = green_payload.parse_bool(
                "tally_green_status",
                #crate_path::command::response::BoolConvention::OnIs03
            )?;
            Ok(#crate_path::command::InquiryData::TallyStatus { red_on, green_on })
        }
    }
}

/// Generate a sharpness mode parser (Auto=0x02, Manual=0x03)
///
/// Maps 0x02 to Auto and 0x03 to Manual (similar to bool convention but returns enum).
pub fn generate_sharpness_mode_parser(crate_path: &TokenStream) -> TokenStream {
    quote! {
        {
            if data.is_empty() {
                return Err(#crate_path::Error::invalid_response_length(1, data));
            }
            let mode = match data[0] {
                0x02 => #crate_path::command::image::SharpnessMode::Auto,
                0x03 => #crate_path::command::image::SharpnessMode::Manual,
                _ => return Err(#crate_path::Error::InvalidParameter {
                    parameter: "sharpness_mode",
                    value: ::std::borrow::Cow::Owned(format!("0x{:02X}", data[0])),
                    reason: ::std::borrow::Cow::Borrowed("Expected 0x02 (Auto) or 0x03 (Manual)"),
                }),
            };
            Ok(#crate_path::command::InquiryData::SharpnessMode { mode })
        }
    }
}

/// Converter method for byte-to-type parsing
#[derive(Debug, Clone, Copy)]
pub enum ConverterMethod {
    /// Use `Type::from_byte(byte)` (infallible)
    FromByte,
    /// Use `Type::try_from(byte)?` (returns Result)
    TryFrom,
    /// Use `Type::new(byte).map_err(...)?` (returns Result with custom type)
    New,
}

/// Generate a byte-to-type converter parser
///
/// Converts a single byte to a type using the specified converter method.
pub fn generate_byte_converter_parser(
    response_variant: &Ident,
    field_name: &Ident,
    converter_type: &TokenStream,
    converter_method: ConverterMethod,
    crate_path: &TokenStream,
) -> TokenStream {
    let param_name = field_name.to_string();

    match converter_method {
        ConverterMethod::FromByte => {
            quote! {
                {
                    if data.is_empty() {
                        return Err(#crate_path::Error::invalid_response_length(1, data));
                    }
                    let #field_name = #converter_type::from_byte(data[0]);
                    Ok(#crate_path::command::InquiryData::#response_variant { #field_name })
                }
            }
        }
        ConverterMethod::TryFrom => {
            quote! {
                {
                    if data.is_empty() {
                        return Err(#crate_path::Error::invalid_response_length(1, data));
                    }
                    let #field_name = #converter_type::try_from(data[0])?;
                    Ok(#crate_path::command::InquiryData::#response_variant { #field_name })
                }
            }
        }
        ConverterMethod::New => {
            quote! {
                {
                    if data.is_empty() {
                        return Err(#crate_path::Error::invalid_response_length(1, data));
                    }
                    let #field_name = #converter_type::new(data[0]).map_err(|_| {
                        #crate_path::Error::InvalidParameter {
                            parameter: #param_name,
                            value: ::std::borrow::Cow::Owned(data[0].to_string()),
                            reason: ::std::borrow::Cow::Borrowed("value out of range"),
                        }
                    })?;
                    Ok(#crate_path::command::InquiryData::#response_variant { #field_name })
                }
            }
        }
    }
}

/// Generate a gamma parser (direct byte value)
pub fn generate_gamma_parser(crate_path: &TokenStream) -> TokenStream {
    quote! {
        {
            if data.is_empty() {
                return Err(#crate_path::Error::invalid_response_length(1, data));
            }
            Ok(#crate_path::command::InquiryData::Gamma { value: data[0] })
        }
    }
}

/// Generate an auto white balance sensitivity parser
///
/// Maps 0x00=High, 0x01=Normal, 0x02=Low
pub fn generate_auto_wb_sensitivity_parser(crate_path: &TokenStream) -> TokenStream {
    quote! {
        {
            if data.is_empty() {
                return Err(#crate_path::Error::invalid_response_length(1, data));
            }
            let sensitivity = match data[0] {
                0x00 => #crate_path::command::AutoWhiteBalanceSensitivity::High,
                0x01 => #crate_path::command::AutoWhiteBalanceSensitivity::Normal,
                0x02 => #crate_path::command::AutoWhiteBalanceSensitivity::Low,
                _ => return Err(#crate_path::Error::InvalidParameter {
                    parameter: "auto_wb_sensitivity",
                    value: ::std::borrow::Cow::Owned(format!("0x{:02X}", data[0])),
                    reason: ::std::borrow::Cow::Borrowed(
                        "Expected 0x00 (High), 0x01 (Normal), or 0x02 (Low)"
                    ),
                }),
            };
            Ok(#crate_path::command::InquiryData::AutoWhiteBalanceSensitivity { sensitivity })
        }
    }
}
