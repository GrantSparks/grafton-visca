//! Tally-related response decoders.

use std::borrow::Cow;

use super::super::payload::Payload;
use crate::{
    command::{
        response::types::{ViscaResponse, ViscaResponseType},
        InquiryResponse,
    },
    error::Error,
};

/// Decode tally-related inquiry responses.
pub(crate) fn decode(
    kind: ViscaResponseType,
    payload: Payload<'_>,
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::TallyRed => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::TallyRed {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        ViscaResponseType::TallyGreen => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::TallyGreen {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        ViscaResponseType::TallyStatus => {
            // Tally light status response
            // Two bytes: first for red, second for green
            // Each byte: 0x02 = off, 0x03 = on
            if payload.len() != 2 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let red_on = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "TallyStatus red",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            let green_on = match payload.as_slice()[1] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "TallyStatus green",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[1])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::TallyStatus {
                red_on,
                green_on,
            })))
        }
        ViscaResponseType::TallyAutoAdjust => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let on = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "TallyAutoAdjust status",
                        value: Cow::Owned(format!("0x{:02X}", payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(ViscaResponse::Inquiry(
                InquiryResponse::TallyAutoAdjust { on },
            )))
        }
        _ => None,
    }
}
