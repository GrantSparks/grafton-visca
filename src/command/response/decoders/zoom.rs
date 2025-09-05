//! Zoom-related response decoders.

use std::borrow::Cow;

use super::super::payload::{Nibbles4Or8, Payload};
use crate::{
    command::{
        response::types::{ViscaResponse, ViscaResponseType},
        InquiryResponse,
    },
    error::Error,
};

/// Decode zoom-related inquiry responses.
pub(crate) fn decode(
    kind: ViscaResponseType,
    payload: Payload<'_>,
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::ZoomPosition => {
            // Standard VISCA expects 4 bytes for zoom position
            // But some cameras may return 8 bytes (possibly including digital zoom info)
            match Nibbles4Or8::try_from(payload) {
                Ok(nibbles) => {
                    // Extended format: Some cameras return 8 bytes
                    // This might include both optical and digital zoom info
                    // For now, use the first 4 bytes as the zoom position
                    if matches!(nibbles, Nibbles4Or8::N8(_)) {
                        tracing::warn!(
                            "ZoomPosition: Received extended format (8 bytes). Using first 4 bytes."
                        );
                    }
                    let position = nibbles.first_u16();
                    Some(Ok(ViscaResponse::Inquiry(InquiryResponse::ZoomPosition {
                        position,
                    })))
                }
                Err(e) => Some(Err(e)),
            }
        }
        ViscaResponseType::ZoomOut => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomOut status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::ZoomOut {
                active,
            })))
        }
        ViscaResponseType::ZoomIn => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let active = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomIn status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (inactive) or 0x03 (active)"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::ZoomIn {
                active,
            })))
        }
        ViscaResponseType::ZoomTeleWide => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let tele = match payload.as_slice()[0] {
                0x02 => false, // Wide active
                0x03 => true,  // Tele active
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomTeleWide status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (wide) or 0x03 (tele)"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::ZoomTeleWide {
                tele,
            })))
        }
        _ => None,
    }
}
