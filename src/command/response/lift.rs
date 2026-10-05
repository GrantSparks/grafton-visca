//! Response lifting from basic protocol layer to high-level types.

use super::{
    decoders::{dispatch, dispatch_with_framing},
    payload::Payload,
    types::Response,
};
use crate::command::{inquiry_structs::InquiryKind, pan_tilt::PanTiltFraming};
use crate::{
    capabilities::{PanTilt, Profile},
    error::Error,
    protocol::response::{decode_basic, BasicKind, BasicResponse},
    PanTiltCoordinateConversion,
};

/// Lift a basic protocol response to a high-level Response.
///
/// A data reply is decoded as `expected`, with `framing` selecting the pan/tilt
/// position layout; without an expected kind it stays [`Response::Unknown`].
pub(crate) fn lift_inquiry(
    basic: &BasicResponse<'_>,
    expected: Option<&InquiryKind>,
    framing: PanTiltFraming,
) -> Result<Response, Error> {
    match basic.kind {
        BasicKind::Ack => Ok(Response::CmdAck {
            socket: basic.socket,
        }),
        BasicKind::Completion => Ok(Response::Completion {
            socket: basic.socket,
        }),
        BasicKind::Error(code) => Ok(Response::Error(Error::from_code(code))),
        BasicKind::NetworkChange => Ok(Response::Unknown {
            response_type: None,
            data: vec![],
        }),
        BasicKind::DataReply => match expected {
            Some(kind) => dispatch_with_framing(*kind, basic.payload, framing),
            None => Ok(Response::Unknown {
                response_type: None,
                data: basic.payload.as_slice().to_vec(),
            }),
        },
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
    dispatch(*expected_type, Payload::new(payload))
}

/// Decodes the basic frame every `Response::parse*` method starts from.
fn decode_frame(bytes: &[u8]) -> Result<BasicResponse<'_>, Error> {
    decode_basic(bytes)
        .ok_or_else(|| Error::invalid_response("valid VISCA response", bytes.to_vec()))
}

impl Response {
    /// Parse a response from raw bytes.
    ///
    /// This method is for parsing basic responses (ACK, Completion, Error).
    /// For inquiry responses, use `parse_with_type` instead.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        lift_inquiry(&decode_frame(bytes)?, None, PanTiltFraming::STANDARD)
    }

    /// Parse an inquiry response with a specific expected type.
    pub fn parse_with_type(bytes: &[u8], response_type: &InquiryKind) -> Result<Self, Error> {
        lift_inquiry(
            &decode_frame(bytes)?,
            Some(response_type),
            PanTiltFraming::STANDARD,
        )
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
        let framing =
            PanTiltFraming::for_conversion(PanTiltCoordinateConversion::for_profile::<P>())?;
        lift_inquiry(&decode_frame(bytes)?, Some(response_type), framing)
    }
}
