//! Pan/Tilt-related response decoders.

use super::super::nibbles::combine_nibbles_i16;
use super::super::payload::Payload;
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
    payload: Payload<'_>,
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::PanTiltPosition => {
            if payload.len() == 8 {
                let pan = combine_nibbles_i16(&payload.as_slice()[0..4]);
                let tilt = combine_nibbles_i16(&payload.as_slice()[4..8]);
                Some(Ok(ViscaResponse::Inquiry(
                    InquiryResponse::PanTiltPosition { pan, tilt },
                )))
            } else if payload.len() == 4 {
                tracing::warn!(
                    "PanTiltPosition: Received compact format (4 bytes). Payload: {payload:02X?}. Treating as home position."
                );
                let pan = if payload.len() >= 2 {
                    ((payload.as_slice()[0] as i16) << 8) | (payload.as_slice()[1] as i16)
                } else {
                    0
                };
                let tilt = if payload.len() >= 4 {
                    ((payload.as_slice()[2] as i16) << 8) | (payload.as_slice()[3] as i16)
                } else {
                    0
                };
                Some(Ok(ViscaResponse::Inquiry(
                    InquiryResponse::PanTiltPosition { pan, tilt },
                )))
            } else {
                tracing::error!(
                    "PanTiltPosition: Invalid response length. Expected 8 or 4 bytes, got {len}. Payload: {payload:02X?}",
                    len = payload.len()
                );
                Some(Err(Error::InvalidResponseLength))
            }
        }
        _ => None,
    }
}
