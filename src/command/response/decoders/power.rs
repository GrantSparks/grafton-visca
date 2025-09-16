//! Power-related response decoders.

use super::super::payload::Payload;
use crate::{
    command::{
        response::types::{Response, ResponseKind},
        InquiryResponse,
    },
    error::Error,
};

/// Decode power-related inquiry responses.
pub(crate) fn decode(kind: ResponseKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        ResponseKind::Power => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::Power {
                on: payload.as_slice()[0] == 0x02,
            })))
        }
        ResponseKind::Standby => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(Response::Inquiry(InquiryResponse::Standby {
                in_standby: payload.as_slice()[0] != 0x02,
            })))
        }
        _ => None,
    }
}
