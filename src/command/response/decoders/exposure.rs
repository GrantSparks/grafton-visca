//! Exposure-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles, Payload};
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
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(
                InquiryData::ExposureCompensationMode {
                    on: payload.as_slice()[0] == 0x02,
                },
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
        InquiryKind::Iris => {
            Some(super::super::parse_iris_last_nibble(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::Brightness => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u16_quad(0);
                Some(Ok(Response::Inquiry(InquiryData::Brightness { position })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::Gain => {
            Some(super::super::parse_gain_last_nibble(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::GainLimit => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::GainLimit {
                limit: payload.as_slice()[0],
            })))
        }
        InquiryKind::IrisControl => {
            Some(super::super::parse_iris_control(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::IrisUp => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "IrisUp status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::IrisUp { active })))
        }
        InquiryKind::IrisDown => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "IrisDown status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::IrisDown { active })))
        }
        _ => None,
    }
}
