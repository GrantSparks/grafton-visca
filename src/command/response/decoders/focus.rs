//! Focus-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles, Payload};
use crate::{
    command::{
        response::types::{Response, ResponseKind},
        AutoFocusSensitivity, FocusMode, FocusRange, FocusZone, InquiryResponse,
    },
    error::Error,
};

/// Decode focus-related inquiry responses.
pub(crate) fn decode(kind: ResponseKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        ResponseKind::FocusPosition => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u16_quad(0);
                Some(Ok(Response::Inquiry(InquiryResponse::FocusPosition {
                    position,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        ResponseKind::FocusNearLimit => match Nibbles::<4>::try_from(payload) {
            Ok(nibbles) => {
                let position = nibbles.u16_quad(0);
                Some(Ok(Response::Inquiry(InquiryResponse::FocusNearLimit {
                    position,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        ResponseKind::FocusZone => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
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
            Some(Ok(Response::Inquiry(InquiryResponse::FocusZone { zone })))
        }
        ResponseKind::AutoFocusSensitivity => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
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
            Some(Ok(Response::Inquiry(
                InquiryResponse::AutoFocusSensitivity { sensitivity },
            )))
        }
        ResponseKind::FocusMode => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
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
            Some(Ok(Response::Inquiry(InquiryResponse::FocusMode { mode })))
        }
        ResponseKind::FocusRange => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            match parse_focus_range(payload.as_slice()) {
                Ok(response) => Some(Ok(Response::Inquiry(response))),
                Err(e) => Some(Err(e)),
            }
        }
        ResponseKind::AutoFocus => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let enabled = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "autofocus_status",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed(
                            "Invalid autofocus status value. Expected 0x02 (off) or 0x03 (on)",
                        ),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryResponse::AutoFocus {
                enabled,
            })))
        }
        ResponseKind::FocusUnlock => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let unlocked =
                match payload.as_slice()[0] {
                    0x02 => false,
                    0x03 => true,
                    _ => return Some(Err(Error::InvalidParameter {
                        parameter: "focus_unlock",
                        value: Cow::Owned(format!("{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed(
                            "Invalid focus unlock value. Expected 0x02 (locked) or 0x03 (unlocked)",
                        ),
                    })),
                };
            Some(Ok(Response::Inquiry(InquiryResponse::FocusUnlock {
                unlocked,
            })))
        }
        ResponseKind::FocusNearFar => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
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
            Some(Ok(Response::Inquiry(InquiryResponse::FocusNearFar {
                near,
            })))
        }
        _ => None,
    }
}

fn parse_focus_range(data: &[u8]) -> Result<InquiryResponse, Error> {
    if data.is_empty() {
        return Err(Error::InvalidResponseLength);
    }
    let range = FocusRange::try_from(data[0])?;
    Ok(InquiryResponse::FocusRange { range })
}
