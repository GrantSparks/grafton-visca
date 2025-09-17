//! Tally-related response decoders.

use std::borrow::Cow;

use super::super::payload::Payload;
use crate::{
    command::{
        response::types::{InquiryKind, Response},
        InquiryData,
    },
    error::Error,
};

/// Decode tally-related inquiry responses.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::TallyRed => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryData::TallyRed {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        InquiryKind::TallyGreen => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryData::TallyGreen {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        InquiryKind::TallyStatus => {
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
            Some(Ok(Response::Inquiry(InquiryData::TallyStatus {
                red_on,
                green_on,
            })))
        }
        InquiryKind::TallyAutoAdjust => {
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
            Some(Ok(Response::Inquiry(InquiryData::TallyAutoAdjust { on })))
        }
        _ => None,
    }
}
