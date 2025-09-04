//! Response lifting from basic protocol layer to high-level types.

use std::borrow::Cow;

use crate::{
    error::Error,
    protocol::response::{decode_basic, BasicKind, BasicResponse},
};

use super::{
    decoders::dispatch,
    types::{ViscaResponse, ViscaResponseType},
};

/// Lift a basic protocol response to a high-level ViscaResponse.
///
/// This function converts from the protocol layer's basic response types
/// to the command layer's semantic response types, optionally parsing
/// inquiry payloads when an expected type is provided.
pub fn lift_inquiry(
    basic: &BasicResponse<'_>,
    expected: Option<&ViscaResponseType>,
) -> Result<ViscaResponse, Error> {
    match basic.kind {
        BasicKind::Ack => Ok(ViscaResponse::CmdAck {
            socket: basic.socket,
        }),
        BasicKind::Completion => Ok(ViscaResponse::Completion {
            socket: basic.socket,
        }),
        BasicKind::Error(code) => Ok(ViscaResponse::Error(Error::from_code(code))),
        BasicKind::NetworkChange => {
            // Network change could be treated as Unknown or a special completion
            Ok(ViscaResponse::Unknown {
                response_type: None,
                data: vec![],
            })
        }
        BasicKind::DataReply => {
            if let Some(response_type) = expected {
                // Use the decoder dispatch to parse the inquiry payload
                dispatch(*response_type, basic.payload)
            } else {
                Ok(ViscaResponse::Unknown {
                    response_type: None,
                    data: basic.payload.to_vec(),
                })
            }
        }
        BasicKind::Unknown => Ok(ViscaResponse::Unknown {
            response_type: None,
            data: basic.payload.to_vec(),
        }),
    }
}

/// Parse inquiry response payload directly without frame reconstruction.
///
/// This is the canonical parser for inquiry payloads, designed to work
/// directly with payload bytes rather than full frames.
pub fn parse_inquiry_payload(
    payload: &[u8],
    expected_type: &ViscaResponseType,
) -> Result<ViscaResponse, Error> {
    dispatch(*expected_type, payload)
}

impl ViscaResponse {
    /// Parse a response from raw bytes.
    ///
    /// This method is for parsing basic responses (ACK, Completion, Error).
    /// For inquiry responses, use `parse_with_type` instead.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        // Use the canonical decoder from protocol::response
        let basic = decode_basic(bytes).ok_or_else(|| Error::InvalidResponse {
            expected: Cow::Borrowed("Valid VISCA response"),
            actual: bytes.to_vec(),
        })?;

        // Convert to ViscaResponse without expected type (for non-inquiry responses)
        lift_inquiry(&basic, None)
    }

    /// Parse an inquiry response with a specific expected type.
    pub fn parse_with_type(bytes: &[u8], response_type: &ViscaResponseType) -> Result<Self, Error> {
        // Use the canonical decoder from protocol::response
        let basic = decode_basic(bytes).ok_or_else(|| Error::InvalidResponse {
            expected: Cow::Borrowed("Valid VISCA response"),
            actual: bytes.to_vec(),
        })?;

        // Convert to ViscaResponse with the expected type for inquiry parsing
        lift_inquiry(&basic, Some(response_type))
    }
}
