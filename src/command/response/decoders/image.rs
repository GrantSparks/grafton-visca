//! Image-related response decoders.

use super::super::payload::{Nibbles, Payload};
use crate::{
    command::{
        image::{BlackWhiteMode, NoiseReductionMode, NoiseReductionSpeed},
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
        InquiryKind::SharpnessPosition => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u16_quad(0);
                Some(Ok(Response::Inquiry(InquiryData::SharpnessPosition {
                    position,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::SharpnessMode => {
            Some(super::super::parse_sharpness_mode(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::Saturation => Some(
            super::super::parse_saturation_last_nibble(payload.as_slice()).map(Response::Inquiry),
        ),
        InquiryKind::Hue => {
            Some(super::super::parse_hue_last_nibble(payload.as_slice()).map(Response::Inquiry))
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
            Some(super::super::parse_picture_effect(payload.as_slice()).map(Response::Inquiry))
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
            Some(super::super::parse_flip_mode(payload.as_slice()).map(Response::Inquiry))
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
            Some(super::super::parse_nd_filter(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::Gamma => {
            Some(super::super::parse_gamma(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::TwoToneMode => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::TwoToneMode {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        InquiryKind::DefogMode => {
            Some(super::super::parse_defog_mode(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::DefogLevel => {
            Some(super::super::parse_defog_level(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::NoiseReductionLevel => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::NoiseReductionLevel(
                payload.as_slice()[0],
            ))))
        }
        _ => None,
    }
}
