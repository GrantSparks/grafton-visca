//! Response lifting from basic protocol layer to high-level types.

use std::borrow::Cow;

use super::{
    decoders::{dispatch, dispatch_for},
    types::Response,
};
use crate::command::inquiry_registry::InquiryKind;
use crate::{
    capabilities::{PanTilt, Profile},
    error::Error,
    protocol::response::{decode_basic, BasicKind, BasicResponse},
};

/// Lift a basic protocol response to a high-level Response.
///
/// This function converts from the protocol layer's basic response types
/// to the command layer's semantic response types, optionally parsing
/// inquiry payloads when an expected type is provided.
pub fn lift_inquiry(
    basic: &BasicResponse<'_>,
    expected: Option<&InquiryKind>,
) -> Result<Response, Error> {
    match basic.kind {
        BasicKind::Ack => Ok(Response::CmdAck {
            socket: basic.socket,
        }),
        BasicKind::Completion => Ok(Response::Completion {
            socket: basic.socket,
        }),
        BasicKind::Error(code) => Ok(Response::Error(Error::from_code(code))),
        BasicKind::NetworkChange => {
            // Network change could be treated as Unknown or a special completion
            Ok(Response::Unknown {
                response_type: None,
                data: vec![],
            })
        }
        BasicKind::DataReply => {
            if let Some(response_type) = expected {
                // Use the decoder dispatch to parse the inquiry payload
                dispatch(*response_type, basic.payload)
            } else {
                Ok(Response::Unknown {
                    response_type: None,
                    data: basic.payload.as_slice().to_vec(),
                })
            }
        }
        BasicKind::Unknown => Ok(Response::Unknown {
            response_type: None,
            data: basic.payload.as_slice().to_vec(),
        }),
    }
}

/// Lift a basic protocol response to a high-level Response with profile awareness.
///
/// This function converts from the protocol layer's basic response types
/// to the command layer's semantic response types, parsing inquiry payloads
/// with profile-specific coordinate system handling when needed.
pub fn lift_inquiry_for<P: Profile + PanTilt>(
    basic: &BasicResponse<'_>,
    expected: Option<&InquiryKind>,
) -> Result<Response, Error> {
    match basic.kind {
        BasicKind::Ack => Ok(Response::CmdAck {
            socket: basic.socket,
        }),
        BasicKind::Completion => Ok(Response::Completion {
            socket: basic.socket,
        }),
        BasicKind::Error(code) => Ok(Response::Error(Error::from_code(code))),
        BasicKind::NetworkChange => {
            // Network change could be treated as Unknown or a special completion
            Ok(Response::Unknown {
                response_type: None,
                data: vec![],
            })
        }
        BasicKind::DataReply => {
            if let Some(response_type) = expected {
                // Use the profile-aware decoder dispatch to parse the inquiry payload
                dispatch_for::<P>(*response_type, basic.payload)
            } else {
                Ok(Response::Unknown {
                    response_type: None,
                    data: basic.payload.as_slice().to_vec(),
                })
            }
        }
        BasicKind::Unknown => Ok(Response::Unknown {
            response_type: None,
            data: basic.payload.as_slice().to_vec(),
        }),
    }
}

/// Parse inquiry response payload directly without frame reconstruction.
///
/// This is the canonical parser for inquiry payloads, designed to work
/// directly with payload bytes rather than full frames.
pub fn parse_inquiry_payload(
    payload: &[u8],
    expected_type: &InquiryKind,
) -> Result<Response, Error> {
    use crate::command::response::payload::Payload;
    dispatch(*expected_type, Payload::new(payload))
}

impl Response {
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

        // Convert to Response without expected type (for non-inquiry responses)
        lift_inquiry(&basic, None)
    }

    /// Parse an inquiry response with a specific expected type.
    pub fn parse_with_type(bytes: &[u8], response_type: &InquiryKind) -> Result<Self, Error> {
        // Use the canonical decoder from protocol::response
        let basic = decode_basic(bytes).ok_or_else(|| Error::InvalidResponse {
            expected: Cow::Borrowed("Valid VISCA response"),
            actual: bytes.to_vec(),
        })?;

        // Convert to Response with the expected type for inquiry parsing
        lift_inquiry(&basic, Some(response_type))
    }

    /// Parse an inquiry response with profile-specific handling.
    ///
    /// This method enables profile-aware parsing for responses that require
    /// coordinate system conversion (e.g., PanTiltPosition on cameras with
    /// unsigned-centered coordinates).
    pub fn parse_with_profile<P: Profile + PanTilt>(
        bytes: &[u8],
        response_type: &InquiryKind,
    ) -> Result<Self, Error> {
        // Use the canonical decoder from protocol::response
        let basic = decode_basic(bytes).ok_or_else(|| Error::InvalidResponse {
            expected: Cow::Borrowed("Valid VISCA response"),
            actual: bytes.to_vec(),
        })?;

        // Convert to Response with profile-aware inquiry parsing
        lift_inquiry_for::<P>(&basic, Some(response_type))
    }
}
