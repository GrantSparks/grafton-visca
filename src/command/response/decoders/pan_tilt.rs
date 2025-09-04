//! Pan/Tilt-related response decoders.

use super::super::nibbles::combine_nibbles_i16;
use crate::{
    command::{
        response::types::{ViscaResponse, ViscaResponseType},
        InquiryResponse,
    },
    error::Error,
};

/// Decode pan/tilt-related inquiry responses.
pub(crate) fn decode(
    kind: ViscaResponseType,
    payload: &[u8],
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::PanTiltPosition => {
            // Standard VISCA expects 8 bytes (4 for pan, 4 for tilt)
            // But some cameras may return 4 bytes with combined values
            if payload.len() == 8 {
                // Standard format: PP PP PP PP TT TT TT TT
                let pan = combine_nibbles_i16(&payload[0..4]);
                let tilt = combine_nibbles_i16(&payload[4..8]);
                Some(Ok(ViscaResponse::Inquiry(
                    InquiryResponse::PanTiltPosition { pan, tilt },
                )))
            } else if payload.len() == 4 {
                // Compact format: Some cameras return PP PP TT TT
                // or all zeros when at home position
                tracing::warn!(
                    "PanTiltPosition: Received compact format (4 bytes). Payload: {payload:02X?}. Treating as home position."
                );
                // For now, treat 4-byte response as home position (0, 0)
                // This may need adjustment based on specific camera models
                let pan = if payload.len() >= 2 {
                    ((payload[0] as i16) << 8) | (payload[1] as i16)
                } else {
                    0
                };
                let tilt = if payload.len() >= 4 {
                    ((payload[2] as i16) << 8) | (payload[3] as i16)
                } else {
                    0
                };
                Some(Ok(ViscaResponse::Inquiry(
                    InquiryResponse::PanTiltPosition { pan, tilt },
                )))
            } else {
                tracing::error!(
                    "PanTiltPosition: Invalid response length. Expected 8 or 4 bytes, got {}. Payload: {:02X?}",
                    payload.len(),
                    payload
                );
                Some(Err(Error::InvalidResponseLength))
            }
        }
        _ => None,
    }
}
