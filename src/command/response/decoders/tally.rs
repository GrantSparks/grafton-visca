//! Tally-related response decoders.

use std::borrow::Cow;

use super::super::payload::Payload;
use crate::{
    command::{
        response::types::{Response, ResponseKind},
        InquiryResponse,
    },
    error::Error,
};

/// Decode tally-related inquiry responses.
pub(crate) fn decode(kind: ResponseKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        ResponseKind::TallyRed => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::TallyRed {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        ResponseKind::TallyGreen => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::TallyGreen {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        ResponseKind::TallyStatus => {
            if payload.len() != 2 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let red_on = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "TallyStatus red",
                        value: Cow::Owned(format!("0x{value:02X}", value = payload.as_slice()[0])),
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
                        value: Cow::Owned(format!("0x{value:02X}", value = payload.as_slice()[1])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryResponse::TallyStatus {
                red_on,
                green_on,
            })))
        }
        ResponseKind::TallyAutoAdjust => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            let on = match payload.as_slice()[0] {
                0x02 => false,
                0x03 => true,
                _ => {
                    return Some(Err(Error::InvalidParameter {
                        parameter: "TallyAutoAdjust status",
                        value: Cow::Owned(format!("0x{value:02X}", value = payload.as_slice()[0])),
                        reason: Cow::Borrowed("Expected 0x02 (off) or 0x03 (on)"),
                    }))
                }
            };
            Some(Ok(Response::Inquiry(InquiryResponse::TallyAutoAdjust {
                on,
            })))
        }
        _ => None,
    }
}
