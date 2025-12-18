//! Focus-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles, Payload};
use crate::{
    command::{
        response::types::{InquiryKind, Response},
        AutoFocusSensitivity, FocusMode, FocusZone, InquiryData,
    },
    error::Error,
};

/// Decode focus-related inquiry responses.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::FocusPosition => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u16_quad(0);
                Some(Ok(Response::Inquiry(InquiryData::FocusPosition {
                    position,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::FocusNearLimit => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u16_quad(0);
                Some(Ok(Response::Inquiry(InquiryData::FocusNearLimit {
                    position,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::FocusZone => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let zone = match payload.as_slice()[0] {
                0x00 => FocusZone::Top,
                0x01 => FocusZone::Center,
                0x02 => FocusZone::Bottom,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "focus_zone",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Unknown focus zone value"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::FocusZone { zone })))
        }
        InquiryKind::AutoFocusSensitivity => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let sensitivity = match payload.as_slice()[0] {
                0x00 => AutoFocusSensitivity::Low,
                0x01 => AutoFocusSensitivity::Normal,
                0x02 => AutoFocusSensitivity::High,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "auto_focus_sensitivity",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Unknown auto focus sensitivity value"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::AutoFocusSensitivity {
                sensitivity,
            })))
        }
        InquiryKind::FocusMode => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let mode = match payload.as_slice()[0] {
                0x02 => FocusMode::Auto,
                0x03 => FocusMode::Manual,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "focus_mode",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Unknown focus mode value"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::FocusMode { mode })))
        }
        InquiryKind::FocusRange => {
            Some(super::super::parse_focus_range(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::AutoFocus => {
            Some(super::super::parse_auto_focus(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::FocusUnlock => {
            Some(super::super::parse_focus_unlock(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::FocusNearFar => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            let near = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "FocusNearFar status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (far) or 0x03 (near)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::FocusNearFar { near })))
        }
        _ => None,
    }
}
