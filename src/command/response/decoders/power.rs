//! Power-related response decoders.

use crate::{
    command::{
        response::types::{ViscaResponse, ViscaResponseType},
        InquiryResponse,
    },
    error::Error,
};

/// Decode power-related inquiry responses.
pub(crate) fn decode(
    kind: ViscaResponseType,
    payload: &[u8],
) -> Option<Result<ViscaResponse, Error>> {
    match kind {
        ViscaResponseType::Power => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Power {
                on: payload[0] == 0x02,
            })))
        }
        ViscaResponseType::Standby => {
            if payload.len() != 1 {
                return Some(Err(Error::InvalidResponseLength));
            }
            // 0x02 = ON, 0x03 = OFF (standby)
            Some(Ok(ViscaResponse::Inquiry(InquiryResponse::Standby {
                in_standby: payload[0] != 0x02, // 0x02 = ON (not in standby), 0x03 = OFF (in standby)
            })))
        }
        _ => None,
    }
}
