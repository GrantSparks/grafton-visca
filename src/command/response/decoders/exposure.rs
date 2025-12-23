//! Exposure-related response decoders.

use std::borrow::Cow;

use super::super::payload::{BoolConvention, Nibbles, Payload};
use crate::{
    command::{
        response::types::{InquiryKind, Response},
        ExposureMode, InquiryData,
    },
    error::Error,
};

/// Decode exposure-related inquiry responses.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::ExposureMode => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let mode = match payload.as_slice()[0] {
                0x00 => ExposureMode::Auto,
                0x03 => ExposureMode::Manual,
                0x0A => ExposureMode::Shutter,
                0x0B => ExposureMode::Iris,
                0x0D => ExposureMode::Bright,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "exposure_mode",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Unknown exposure mode value"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::ExposureMode { mode })))
        }
        InquiryKind::ExposureCompensationMode => {
            // ExposureCompensationMode uses inverted convention (0x02 = on)
            let on = match payload.parse_bool("exposure_compensation_mode", BoolConvention::OnIs02)
            {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(
                InquiryData::ExposureCompensationMode { on },
            )))
        }
        InquiryKind::ExposureCompensation => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let raw_value = nibbles.u8_pair(2);
                Some(Ok(Response::Inquiry(InquiryData::ExposureCompensation {
                    value: raw_value as i8 - 7,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::ExposureCompensationPosition => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = crate::types::ExposureCompensationPosition::new(nibbles.u16_quad(0));
                Some(Ok(Response::Inquiry(
                    InquiryData::ExposureCompensationPosition { position },
                )))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::Shutter => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u8_pair(2) as u16;
                Some(Ok(Response::Inquiry(InquiryData::Shutter { position })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::Iris => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => Some(Ok(Response::Inquiry(InquiryData::Iris {
                position: nibbles.last_nibble(),
            }))),
            Err(e) => Some(Err(e)),
        },
        InquiryKind::Brightness => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u16_quad(0);
                Some(Ok(Response::Inquiry(InquiryData::Brightness { position })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::Gain => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => Some(Ok(Response::Inquiry(InquiryData::GainLevel {
                gain: nibbles.last_nibble(),
            }))),
            Err(e) => Some(Err(e)),
        },
        InquiryKind::GainLimit => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::GainLimit {
                limit: payload.as_slice()[0],
            })))
        }
        InquiryKind::IrisControl => {
            let auto = match payload.parse_bool("iris_control", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::IrisControl { auto })))
        }
        InquiryKind::IrisUp => {
            let active = match payload.parse_bool("iris_up_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::IrisUp { active })))
        }
        InquiryKind::IrisDown => {
            let active = match payload.parse_bool("iris_down_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::IrisDown { active })))
        }
        _ => None,
    }
}
