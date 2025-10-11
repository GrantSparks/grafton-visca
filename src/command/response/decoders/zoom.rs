//! Zoom-related response decoders.
//!
//! # VISCA Zoom Position Specification
//!
//! Per the VISCA specification, zoom position inquiries return **16-bit values**
//! encoded as 4 nibbles (bytes with values 0x00-0x0F):
//! - Response format: `90 50 0p 0q 0r 0s FF`
//! - Position value: `(p << 12) | (q << 8) | (r << 4) | s`
//! - Most cameras use range `0x0000` to `0x4000` for optical zoom
//!
//! Some vendor devices may send extended 8-nibble responses. When detected,
//! this decoder logs a warning and uses only the first 16 bits per spec.

use std::borrow::Cow;

use super::super::payload::{Nibbles4Or8, Payload};
use crate::{
    command::{
        response::types::{InquiryKind, Response},
        InquiryData,
    },
    error::Error,
};

/// Decode zoom-related inquiry responses.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::ZoomPosition => match Nibbles4Or8::try_from(payload) {
            Ok(nibbles) => {
                // Per VISCA spec, zoom position is 16-bit (4 nibbles).
                // Some devices send 8 nibbles; we use only the first 4 per spec.
                if matches!(nibbles, Nibbles4Or8::N8(_)) {
                    tracing::warn!(
                        "ZoomPosition: Received extended format (8 nibbles). Using first 4 nibbles (16-bit) per VISCA spec."
                    );
                }
                let position = nibbles.first_u16();
                Some(Ok(Response::Inquiry(InquiryData::ZoomPosition {
                    position,
                })))
            }
            Err(e) => Some(Err(e)),
        },
        InquiryKind::ZoomOut => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomOut status",
                        value: Cow::Owned(format!("0x{byte:02X}", byte = payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::ZoomOut { active })))
        }
        InquiryKind::ZoomIn => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomIn status",
                        value: Cow::Owned(format!("0x{byte:02X}", byte = payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::ZoomIn { active })))
        }
        InquiryKind::ZoomTeleWide => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let tele = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomTeleWide status",
                        value: Cow::Owned(format!("0x{byte:02X}", byte = payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (wide) or 0x03 (tele)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryData::ZoomTeleWide { tele })))
        }
        _ => None,
    }
}
