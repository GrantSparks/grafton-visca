//! Tally-related response decoders.

use super::super::payload::{BoolConvention, Payload};
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
            // TallyRed uses inverted convention (0x02 = on)
            let on = match payload.parse_bool("tally_red_status", BoolConvention::OnIs02) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::TallyRed { on })))
        }
        InquiryKind::TallyGreen => {
            // TallyGreen uses inverted convention (0x02 = on)
            let on = match payload.parse_bool("tally_green_status", BoolConvention::OnIs02) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::TallyGreen { on })))
        }
        InquiryKind::TallyStatus => {
            // TallyStatus has 2 bytes: red then green, both use standard convention
            if payload.len() < 2 {
                return Some(Err(Error::invalid_response_length(2, payload.as_slice())));
            }
            let red_payload = Payload::new(&payload.as_slice()[0..1]);
            let green_payload = Payload::new(&payload.as_slice()[1..2]);
            let red_on = match red_payload.parse_bool("tally_red_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let green_on =
                match green_payload.parse_bool("tally_green_status", BoolConvention::OnIs03) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
            Some(Ok(Response::Inquiry(InquiryData::TallyStatus {
                red_on,
                green_on,
            })))
        }
        InquiryKind::TallyAutoAdjust => {
            let on = match payload.parse_bool("tally_auto_adjust_status", BoolConvention::OnIs03) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Response::Inquiry(InquiryData::TallyAutoAdjust { on })))
        }
        _ => None,
    }
}
