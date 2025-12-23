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

use super::super::payload::{BoolConvention, Nibbles4Or8, Payload};
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
            let active = match payload.parse_bool("zoom_out_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::ZoomOut { active })))
        }
        InquiryKind::ZoomIn => {
            let active = match payload.parse_bool("zoom_in_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::ZoomIn { active })))
        }
        InquiryKind::ZoomTeleWide => {
            let tele = match payload.parse_bool("zoom_tele_wide_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::ZoomTeleWide { tele })))
        }
        _ => None,
    }
}
