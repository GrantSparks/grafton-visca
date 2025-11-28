//! Power-related response decoders.

use crate::{
    command::{
        response::{
            payload::Payload,
            types::{InquiryKind, Response},
        },
        InquiryData,
    },
    error::Error,
};

/// Decode power-related inquiry responses.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::Power => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::Power {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        InquiryKind::Standby => {
            if payload.len() != 1 {
                return Some(Err(Error::invalid_response_length(1, payload.as_slice())));
            }
            Some(Ok(Response::Inquiry(InquiryData::Standby {
                in_standby: payload.as_slice()[0] != 0x02,
            })))
        }
        _ => None,
    }
}
