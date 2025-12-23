//! Image-related response decoders.

use super::super::payload::{BoolConvention, Nibbles, Payload};
use crate::{
    command::{
        image::{BlackWhiteMode, NoiseReductionMode, NoiseReductionSpeed, SharpnessMode},
        resolution::{NdFilterPosition, PictureEffectMode},
        response::types::{InquiryKind, Response},
        InquiryData,
    },
    error::Error,
    types::DefogLevel,
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
            // SharpnessMode uses: 0x02 = Auto, 0x03 = Manual
            // We interpret this with OnIs03 where true means Manual
            if payload.is_empty() {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let mode = match payload.as_slice()[0] {
                0x02 => SharpnessMode::Auto,
                0x03 => SharpnessMode::Manual,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "sharpness_mode",
                        value: std::borrow::Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: std::borrow::Cow::Borrowed("Expected 0x02 (Auto) or 0x03 (Manual)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::SharpnessMode { mode })))
        }
        InquiryKind::Saturation => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => Some(Ok(Response::Inquiry(InquiryData::Saturation {
                level: nibbles.last_nibble(),
            }))),
            Err(e) => Some(Err(e)),
        },
        InquiryKind::Hue => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => Some(Ok(Response::Inquiry(InquiryData::Hue {
                hue: nibbles.last_nibble(),
            }))),
            Err(e) => Some(Err(e)),
        },
        InquiryKind::Contrast => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::Contrast(
                payload.as_slice()[0],
            ))))
        }
        InquiryKind::PictureEffect => {
            if payload.is_empty() {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::PictureEffect {
                effect: PictureEffectMode::from_byte(payload.as_slice()[0]),
            })))
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
            if payload.is_empty() {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let mode = payload.as_slice()[0];
            let horizontal = (mode & 0x01) != 0;
            let vertical = (mode & 0x02) != 0;
            Some(Ok(Response::Inquiry(InquiryData::FlipState {
                horizontal,
                vertical,
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
            // Backlight uses inverted convention (0x02 = on/true)
            let status = match payload.parse_bool("backlight_status", BoolConvention::OnIs02) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::Backlight { status })))
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
            if payload.is_empty() {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::NdFilter {
                position: NdFilterPosition::from_byte(payload.as_slice()[0]),
            })))
        }
        InquiryKind::Gamma => {
            if payload.is_empty() {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::Gamma {
                value: payload.as_slice()[0],
            })))
        }
        InquiryKind::TwoToneMode => {
            // TwoToneMode uses inverted convention (0x02 = on)
            let on = match payload.parse_bool("two_tone_mode_status", BoolConvention::OnIs02) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::TwoToneMode { on })))
        }
        InquiryKind::DefogMode => {
            let enabled = match payload.parse_bool("defog_mode", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::DefogMode { enabled })))
        }
        InquiryKind::DefogLevel => {
            if payload.is_empty() {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let level = match DefogLevel::new(payload.as_slice()[0]) {
                Ok(l) => l,
                Err(_) => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "defog_level",
                        value: std::borrow::Cow::Owned(payload.as_slice()[0].to_string()),
                        reason: std::borrow::Cow::Borrowed("value out of range (0-5)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::DefogLevel { level })))
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
