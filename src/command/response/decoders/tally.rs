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
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::TallyRed {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        InquiryKind::TallyGreen => {
            Some(super::super::parse_tally_green(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::TallyStatus => {
            Some(super::super::parse_tally_status(payload.as_slice()).map(Response::Inquiry))
        }
        InquiryKind::TallyAutoAdjust => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
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
            Some(Ok(Response::Inquiry(InquiryData::TallyAutoAdjust { on })))
        }
        _ => None,
    }
}
