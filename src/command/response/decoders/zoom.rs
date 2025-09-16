//! Zoom-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles4Or8, Payload};
use crate::{
    command::{
        response::types::{Response, ResponseKind},
        InquiryResponse,
    },
    error::Error,
};

/// Decode zoom-related inquiry responses.
pub(crate) fn decode(kind: ResponseKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        ResponseKind::ZoomPosition => match Nibbles4Or8::try_from(payload) {
            Ok(nibbles) => {
                if matches!(nibbles, Nibbles4Or8::N8(_)) {
                    tracing::warn!(
                        "ZoomPosition: Received extended format (8 bytes). Using first 4 bytes."
                    );
                }
                let position = nibbles.first_u16();
                Some(Ok(Response::Inquiry(InquiryResponse::ZoomPosition {
                    position,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        ResponseKind::ZoomOut => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomOut status",
                        value: Cow::Owned(format!("0x{value:02X}", value = payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryResponse::ZoomOut { active })))
        }
        ResponseKind::ZoomIn => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomIn status",
                        value: Cow::Owned(format!("0x{value:02X}", value = payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryResponse::ZoomIn { active })))
        }
        ResponseKind::ZoomTeleWide => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let tele = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomTeleWide status",
                        value: Cow::Owned(format!("0x{value:02X}", value = payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (wide) or 0x03 (tele)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryResponse::ZoomTeleWide {
                tele,
            })))
        }
        _ => None,
    }
}
