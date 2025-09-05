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
                        value: Cow::Owned(format!("{value:02X}", value = payload.as_slice()[0])),
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
            let level = payload.as_slice()[3];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Saturation {
                level,
            })))
        }
        ViscaResponseType::Hue => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
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
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
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
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let level = payload.as_slice()[0];
            Some(Ok(ViscaResponse::Inquiry(
                InquiryResponse::NoiseReduction2D { level },
            )))
        }
        ViscaResponseType::NoiseReduction3D => {
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
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let mode = payload.as_slice()[0];
            let horizontal = (mode & 0x01) != 0;
            let vertical = (mode & 0x02) != 0;
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::FlipMode {
                horizontal,
                vertical,
            })))
        }
        ViscaResponseType::DynamicRange => {
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
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let position = payload.as_slice()[0];
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::NdFilter {
                position,
            })))
        }
        ViscaResponseType::Gamma => {
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
