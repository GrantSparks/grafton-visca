//! Zoom-related response decoders.

use std::borrow::Cow;

use super::super::nibbles::combine_nibbles_u16;
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
    payload: &[u8],
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::ZoomPosition => {
            // Standard VISCA expects 4 bytes for zoom position
            // But some cameras may return 8 bytes (possibly including digital zoom info)
            if payload.len() == 4 {
                // Standard format: 0p 0q 0r 0s
                let position = combine_nibbles_u16(&payload[0..4]);
                Some(Ok(ViscaResponse::Inquiry(InquiryResponse::ZoomPosition {
                    position,
                })))
            } else if payload.len() == 8 {
                // Extended format: Some cameras return 8 bytes
                // This might include both optical and digital zoom info
                // For now, use the first 4 bytes as the zoom position
                tracing::warn!(
                    "ZoomPosition: Received extended format (8 bytes). Payload: {payload:02X?}. Using first 4 bytes."
                );
                let position = combine_nibbles_u16(&payload[0..4]);
                Some(Ok(ViscaResponse::Inquiry(InquiryResponse::ZoomPosition {
                    position,
                })))
            } else {
                Some(Err(Error::InvalidResponseLength))
            }
        }
        ViscaResponseType::ZoomOut => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let active = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomOut status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
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
            let active = match payload[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomIn status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
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
            let tele = match payload[0] {
                0x02 => false, // Wide active
                0x03 => true,  // Tele active
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "ZoomTeleWide status",
                        value: Cow::Owned(format!("0x{:02X}", payload[0])),
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
