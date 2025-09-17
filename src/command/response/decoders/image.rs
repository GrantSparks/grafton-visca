//! Image-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles, Payload};
use crate::{
    command::{
        image::{BlackWhiteMode, NoiseReductionMode, NoiseReductionSpeed, SharpnessMode},
        response::types::{Response, ResponseKind},
        InquiryResponse,
    },
    error::Error,
};

/// Decode image-related inquiry responses.
pub(crate) fn decode(kind: ResponseKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        ResponseKind::Sharpness => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let value = nibbles.u8_pair(2);
                Some(Ok(Response::Inquiry(InquiryResponse::Sharpness { value })))
            }
            Err(e) => Some(Err(e)),
        },
        ResponseKind::SharpnessMode => {
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
            Some(Ok(Response::Inquiry(InquiryResponse::SharpnessMode {
                mode,
            })))
        }
        ResponseKind::Saturation => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let level = payload.as_slice()[3];
            Some(Ok(Response::Inquiry(InquiryResponse::Saturation { level })))
        }
        ResponseKind::Hue => {
            if payload.len() != 4 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let hue = payload.as_slice()[3];
            Some(Ok(Response::Inquiry(InquiryResponse::Hue { hue })))
        }
        ResponseKind::Contrast => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::Contrast(
                payload.as_slice()[0],
            ))))
        }
        ResponseKind::PictureEffect => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let effect = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryResponse::PictureEffect {
                effect,
            })))
        }
        ResponseKind::BlackWhite => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::BlackWhite {
                on: payload.as_slice()[0] == 0x04,
            })))
        }
        ResponseKind::BlackWhiteMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let mode = match BlackWhiteMode::try_from(payload.as_slice()[0]) {
                Ok(mode) => mode,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryResponse::BlackWhiteMode {
                mode,
            })))
        }
        ResponseKind::NoiseReduction2D => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let level = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryResponse::NoiseReduction2D {
                level,
            })))
        }
        ResponseKind::NoiseReduction3D => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let level = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryResponse::NoiseReduction3D {
                level,
            })))
        }
        ResponseKind::NoiseReductionMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let mode = match NoiseReductionMode::try_from(payload.as_slice()[0]) {
                Ok(mode) => mode,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryResponse::NoiseReductionMode {
                mode,
            })))
        }
        ResponseKind::NoiseReductionSpeed => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let speed = match NoiseReductionSpeed::try_from(payload.as_slice()[0]) {
                Ok(speed) => speed,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(
                InquiryResponse::NoiseReductionSpeed { speed },
            )))
        }
        ResponseKind::FlipState => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let value = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryResponse::FlipState {
                horizontal: (value & 0x01) != 0,
                vertical: (value & 0x02) != 0,
            })))
        }
        ResponseKind::DynamicRange => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let level = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryResponse::DynamicRange {
                level,
            })))
        }
        ResponseKind::Backlight => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::Backlight {
                status: payload.as_slice()[0] == 0x02,
            })))
        }
        ResponseKind::Luminance => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::Luminance(
                payload.as_slice()[0],
            ))))
        }
        ResponseKind::NdFilter => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let position = payload.as_slice()[0];
            Some(Ok(Response::Inquiry(InquiryResponse::NdFilter {
                position,
            })))
        }
        ResponseKind::Gamma => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::Gamma {
                value: payload.as_slice()[0],
            })))
        }
        ResponseKind::TwoToneMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::TwoToneMode {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        _ => None,
    }
}
