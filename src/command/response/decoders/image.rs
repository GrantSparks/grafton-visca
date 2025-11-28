//! Image-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles, Payload};
use crate::{
    command::{
        image::{BlackWhiteMode, NoiseReductionMode, NoiseReductionSpeed, SharpnessMode},
        response::types::{InquiryKind, Response},
        InquiryData,
    },
    error::Error,
};

/// Decode image-related inquiry responses.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::Sharpness => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let value = nibbles.u8_pair(2);
                Some(Ok(Response::Inquiry(InquiryData::Sharpness { value })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::SharpnessMode => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
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
            Some(Ok(Response::Inquiry(InquiryData::SharpnessMode { mode })))
        }
        InquiryKind::Saturation => {
            if payload.len() != 4 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let level = payload.as_slice()[3];
            Some(Ok(Response::Inquiry(InquiryData::Saturation { level })))
        }
        InquiryKind::Hue => {
            if payload.len() != 4 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let hue = payload.as_slice()[3];
            Some(Ok(Response::Inquiry(InquiryData::Hue { hue })))
        }
        InquiryKind::Contrast => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::Contrast(
                payload.as_slice()[0],
            ))))
        }
        InquiryKind::PictureEffect => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let effect =
                crate::command::resolution::PictureEffectMode::from_byte(payload.as_slice()[0]);
            Some(Ok(Response::Inquiry(InquiryData::PictureEffect { effect })))
        }
        InquiryKind::BlackWhite => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::BlackWhite {
                on: payload.as_slice()[0] == 0x04,
            })))
        }
        InquiryKind::BlackWhiteMode => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let mode = match BlackWhiteMode::try_from(payload.as_slice()[0]) {
                Ok(mode) => mode,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::BlackWhiteMode { mode })))
        }
        InquiryKind::NoiseReduction2D => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let level = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryData::NoiseReduction2D {
                level,
            })))
        }
        InquiryKind::NoiseReduction3D => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let level = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryData::NoiseReduction3D {
                level,
            })))
        }
        InquiryKind::NoiseReductionMode => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let mode = match NoiseReductionMode::try_from(payload.as_slice()[0]) {
                Ok(mode) => mode,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::NoiseReductionMode {
                mode,
            })))
        }
        InquiryKind::NoiseReductionSpeed => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let speed = match NoiseReductionSpeed::try_from(payload.as_slice()[0]) {
                Ok(speed) => speed,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::NoiseReductionSpeed {
                speed,
            })))
        }
        InquiryKind::FlipState => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let value = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryData::FlipState {
                horizontal: (value & 0x01) != 0,
                vertical: (value & 0x02) != 0,
            })))
        }
        InquiryKind::DynamicRange => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let level = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryData::DynamicRange { level })))
        }
        InquiryKind::Backlight => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::Backlight {
                status: payload.as_slice()[0] == 0x02,
            })))
        }
        InquiryKind::Luminance => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::Luminance(
                payload.as_slice()[0],
            ))))
        }
        InquiryKind::NdFilter => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let position =
                crate::command::resolution::NdFilterPosition::from_byte(payload.as_slice()[0]);
            Some(Ok(Response::Inquiry(InquiryData::NdFilter { position })))
        }
        InquiryKind::Gamma => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::Gamma {
                value: payload.as_slice()[0],
            })))
        }
        InquiryKind::TwoToneMode => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::TwoToneMode {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        _ => None,
    }
}
