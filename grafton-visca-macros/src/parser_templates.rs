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
    // Handle both tuple variants (Luminance(u8)) and struct variants (GainLimit { limit: u8 })
    match response_variant.to_string().as_str() {
        "Luminance" | "Contrast" => {
            quote! {
                Ok(#crate_path::command::InquiryData::#response_variant(data[0]))
            }
        }
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
                Ok(#crate_path::command::InquiryData::#response_variant { preset: data[0] })
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
                return Err(#crate_path::Error::InvalidResponseLength);
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
                return Err(#crate_path::Error::InvalidResponseLength);
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
                return Err(#crate_path::Error::InvalidResponseLength);
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
