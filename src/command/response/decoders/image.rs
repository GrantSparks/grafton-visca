//! Image-related response decoders.

use std::borrow::Cow;

use super::super::nibbles::combine_nibbles_u8;
use super::super::payload::Payload;
use crate::{
    command::{
        image::{BlackWhiteMode, NrMode, NrSpeed, SharpnessMode},
        response::types::{ViscaResponse, ViscaResponseType},
        InquiryResponse,
    },
    error::Error,
};

/// Decode image-related inquiry responses.
pub(crate) fn decode(
    kind: ViscaResponseType,
    payload: Payload<'_>,
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::Sharpness => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let value = combine_nibbles_u8(&payload.as_slice()[2..4]);
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Sharpness {
                value,
            })))
        }
        ViscaResponseType::SharpnessMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let mode = match payload.as_slice()[0] {
                0x02 => SharpnessMode::Auto,
                0x03 => SharpnessMode::Manual,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "sharpness_mode",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Unknown sharpness mode value"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::SharpnessMode {
                mode,
            })))
        }
        ViscaResponseType::Saturation => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // Extract the saturation level from the last nibble
            let level = payload.as_slice()[3];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Saturation {
                level,
            })))
        }
        ViscaResponseType::Hue => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // Extract the hue value from the last nibble
            let hue = payload.as_slice()[3];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Hue { hue })))
        }
        ViscaResponseType::Contrast => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Contrast(
                payload.as_slice()[0],
            ))))
        }
        ViscaResponseType::PictureEffect => {
            // Picture effect inquiry response
            // Single byte indicating current effect
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // Picture effect values:
            // 0x00 = Off (normal)
            // 0x01 = Negative
            // 0x02 = B&W
            // Other values are camera-specific effects
            let effect = payload.as_slice()[0];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::PictureEffect {
                effect,
            })))
        }
        ViscaResponseType::BlackWhite => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::BlackWhite {
                on: payload.as_slice()[0] == 0x04,
            })))
        }
        ViscaResponseType::BlackWhiteMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let mode = match BlackWhiteMode::try_from(payload.as_slice()[0]) {
                Ok(mode) => mode,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(ViscaResponse::Inquiry(
                InquiryResponse::BlackWhiteMode { mode },
            )))
        }
        ViscaResponseType::NoiseReduction2D => {
            // 2D noise reduction level response
            // Based on common VISCA patterns, expecting single byte level value
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let level = payload.as_slice()[0];
            Some(Ok(ViscaResponse::Inquiry(
                InquiryResponse::NoiseReduction2D { level },
            )))
        }
        ViscaResponseType::NoiseReduction3D => {
            // 3D noise reduction level response
            // Based on common VISCA patterns, expecting single byte level value
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let level = payload.as_slice()[0];
            Some(Ok(ViscaResponse::Inquiry(
                InquiryResponse::NoiseReduction3D { level },
            )))
        }
        ViscaResponseType::NrMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let mode = match NrMode::try_from(payload.as_slice()[0]) {
                Ok(mode) => mode,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::NrMode { mode })))
        }
        ViscaResponseType::NrSpeed => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let speed = match NrSpeed::try_from(payload.as_slice()[0]) {
                Ok(speed) => speed,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::NrSpeed {
                speed,
            })))
        }
        ViscaResponseType::ImageFlip => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let value = payload.as_slice()[0];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::ImageFlip {
                horizontal: (value & 0x01) != 0,
                vertical: (value & 0x02) != 0,
            })))
        }
        ViscaResponseType::FlipMode => {
            // Combined flip mode inquiry response
            // Single byte encoding both horizontal and vertical flip
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // Flip mode encoding:
            // 0x00 = No flip
            // 0x01 = Horizontal flip only
            // 0x02 = Vertical flip only
            // 0x03 = Both horizontal and vertical flip
            let mode = payload.as_slice()[0];
            let horizontal = (mode & 0x01) != 0;
            let vertical = (mode & 0x02) != 0;
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::FlipMode {
                horizontal,
                vertical,
            })))
        }
        ViscaResponseType::DynamicRange => {
            // Dynamic range level response
            // Single byte level value (0x0=0 to 0x8=8)
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let level = payload.as_slice()[0];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::DynamicRange {
                level,
            })))
        }
        ViscaResponseType::Backlight => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Backlight {
                status: payload.as_slice()[0] == 0x02,
            })))
        }
        ViscaResponseType::Luminance => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Luminance(
                payload.as_slice()[0],
            ))))
        }
        ViscaResponseType::NdFilter => {
            // ND filter inquiry response
            // Single byte indicating current ND filter position
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // ND filter values:
            // 0x00 = Through (no filter)
            // 0x01 = 1/4 ND
            // 0x02 = 1/8 ND
            // 0x03 = 1/16 ND
            // 0x04 = 1/32 ND
            // 0x05 = 1/64 ND
            let position = payload.as_slice()[0];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::NdFilter {
                position,
            })))
        }
        ViscaResponseType::Gamma => {
            // Gamma curve setting inquiry response
            // Single byte: gamma setting (0=Standard, 1-4=different curves)
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Gamma {
                value: payload.as_slice()[0],
            })))
        }
        ViscaResponseType::TwoToneMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::TwoToneMode {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        _ => None,
    }
}
