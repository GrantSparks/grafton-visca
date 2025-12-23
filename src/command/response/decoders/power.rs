//! Power-related response decoders.

use crate::{
    command::{
        response::{
            payload::{BoolConvention, Payload},
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
            // Power uses inverted convention (0x02 = on)
            let on = match payload.parse_bool("power_status", BoolConvention::OnIs02) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::Power { on })))
        }
        InquiryKind::Standby => {
            let in_standby = match payload.parse_bool("standby_mode", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::Standby { in_standby })))
        }
        _ => None,
    }
}
