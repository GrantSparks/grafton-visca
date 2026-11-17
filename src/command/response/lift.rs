//! Response lifting from basic protocol layer to high-level types.

use super::{
    decoders::{dispatch, dispatch_with_framing},
    payload::Payload,
    types::Response,
};
use crate::command::{inquiry_structs::InquiryKind, pan_tilt_wire::PanTiltFraming};
use crate::{
    error::Error,
    protocol::response::{decode_basic, BasicKind, BasicResponse},
    CompileTimeProfile, ProfileSpec,
};

/// Lift a basic protocol response to a high-level Response.
///
/// A data reply is decoded as `expected`, with `framing` selecting the pan/tilt
/// position layout; without an expected kind it stays [`crate::wire::Response::Unknown`].
pub(crate) fn lift_inquiry(
    basic: &BasicResponse<'_>,
    expected: Option<&InquiryKind>,
    framing: PanTiltFraming,
) -> Result<Response, Error> {
    match basic.kind {
        BasicKind::Ack { socket } => Ok(Response::CmdAck { socket }),
        BasicKind::Completion { socket } => Ok(Response::Completion { socket }),
        BasicKind::Error { code, .. } => Ok(Response::Error(code.into())),
        BasicKind::NetworkChange => Ok(Response::NetworkChange),
        BasicKind::DataReply(payload) => match expected {
            Some(kind) => dispatch_with_framing(*kind, payload, framing),
            None => Ok(Response::Unknown {
                response_type: None,
                data: payload.as_slice().to_vec(),
            }),
        },
        BasicKind::Unknown(payload) => Ok(Response::Unknown {
            response_type: None,
            data: payload.as_slice().to_vec(),
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
    /// This method is for parsing basic responses (ACK, Completion, Error, NetworkChange).
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
    /// unsigned-centered coordinates). Built-in profiles read the registry's
    /// constant framing directly; custom profiles construct and validate their
    /// declared facts.
    pub fn parse_with_profile<P: CompileTimeProfile>(
        bytes: &[u8],
        response_type: &InquiryKind,
    ) -> Result<Self, Error> {
        let coordinates = match P::ID {
            Some(id) => id.pan_tilt_coordinates(),
            None => ProfileSpec::from_compile_time::<P>()?.pan_tilt_coordinates(),
        };
        let framing = match coordinates {
            Some(conversion) => PanTiltFraming::for_conversion(conversion),
            None if matches!(response_type, InquiryKind::PanTiltPosition) => {
                return Err(Error::FeatureNotSupported {
                    feature: "pan/tilt position inquiry",
                });
            }
            None => PanTiltFraming::STANDARD,
        };
        lift_inquiry(&decode_frame(bytes)?, Some(response_type), framing)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        command::{encode::WireEncode, pan_tilt_wire::PanTiltProfiled},
        profiles::{ProfileId, PtzOpticsG2, SonyBrc300, SonyFr7},
        types::{PanSpeed, TiltSpeed},
    };

    fn round_trip<P: CompileTimeProfile>(pan: i32, tilt: i32, response: &[u8]) {
        let coordinates = P::ID
            .expect("built-in identity")
            .pan_tilt_coordinates()
            .expect("pan/tilt coordinates");
        let framing = PanTiltFraming::for_conversion(coordinates);
        let command = PanTiltProfiled::AbsolutePosition {
            framing,
            pan,
            tilt,
            speeds: framing
                .move_speeds(PanSpeed::MIN, TiltSpeed::MIN)
                .expect("paired speeds"),
        };
        let mut encoded = [0; 17];
        let length = command
            .write_into(crate::CameraId::CAMERA_1, &mut encoded)
            .expect("position frame");
        let words = framing.position_nibbles();
        assert_eq!(
            &encoded[length - words - 1..length - 1],
            &response[2..response.len() - 1]
        );
        let parsed = Response::parse_with_profile::<P>(response, &InquiryKind::PanTiltPosition)
            .expect("profile-aware position response");
        assert!(
            matches!(parsed, Response::Inquiry(crate::command::inquiry_structs::InquiryData::PanTiltPosition {
            pan: decoded_pan, tilt: decoded_tilt,
        }) if (decoded_pan, decoded_tilt) == (pan, tilt))
        );
    }

    #[test]
    fn parse_with_profile_round_trips_all_three_position_framings() {
        round_trip::<PtzOpticsG2>(
            -1234,
            5678,
            &[0x90, 0x50, 0xF, 0xB, 2, 0xE, 1, 6, 2, 0xE, 0xFF],
        );
        round_trip::<SonyFr7>(
            -40000,
            46000,
            &[0x90, 0x50, 0xF, 6, 3, 0xC, 0, 0, 0xB, 3, 0xB, 0, 0xFF],
        );
        round_trip::<SonyBrc300>(
            -40000,
            -1000,
            &[0x90, 0x50, 0xF, 6, 3, 0xC, 0, 0xF, 0xC, 1, 8, 0xFF],
        );
    }

    #[test]
    fn constant_registry_coordinates_equal_validated_profile_facts() {
        for &id in ProfileId::all() {
            assert_eq!(
                id.pan_tilt_coordinates(),
                id.spec().expect("profile").pan_tilt_coordinates()
            );
        }
    }
}
