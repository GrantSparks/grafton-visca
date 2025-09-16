//! Pan/Tilt-related response decoders.

use crate::{
    capabilities::{PanTilt, Profile},
    command::{
        response::types::{Response, ResponseKind},
        InquiryResponse,
    },
    error::Error,
};

use super::super::payload::{Nibbles, Payload};

/// Decode pan/tilt-related inquiry responses.
pub(crate) fn decode(kind: ResponseKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        ResponseKind::PanTiltPosition => {
            if payload.len() == 8 {
                match Nibbles::<8>::try_from(payload) {
                    Ok(nibbles) => {
                        let pan = nibbles.i16_quad(0);
                        let tilt = nibbles.i16_quad(4);
                        Some(Ok(Response::Inquiry(InquiryResponse::PanTiltPosition {
                            pan,
                            tilt,
                        })))
                    }
                    Err(e) => Some(Err(e)),
                }
            } else if payload.len() == 4 {
                tracing::warn!(
                    "PanTiltPosition: Received compact format (4 bytes). Payload: {payload:02X?}. Treating as home position."
                );
                let pan = if payload.len() >= 2 {
                    ((payload.as_slice()[0] as i16) << 8) | (payload.as_slice()[1] as i16)
                } else {
                    0
                };
                let tilt = if payload.len() >= 4 {
                    ((payload.as_slice()[2] as i16) << 8) | (payload.as_slice()[3] as i16)
                } else {
                    0
                };
                Some(Ok(Response::Inquiry(InquiryResponse::PanTiltPosition {
                    pan,
                    tilt,
                })))
            } else {
                tracing::error!(
                    "PanTiltPosition: Invalid response length. Expected 8 or 4 bytes, got {len}. Payload: {payload:02X?}",
                    len = payload.len()
                );
                Some(Err(Error::InvalidResponseLength))
            }
        }
        _ => None,
    }
}

/// Profile-aware decode for pan/tilt-related inquiry responses.
///
/// This function uses the profile's coordinate system to convert camera
/// coordinates to logical coordinates.
pub(crate) fn decode_for<P: Profile + PanTilt>(
    kind: ResponseKind,
    payload: Payload<'_>,
) -> Option<Result<Response, Error>> {
    match kind {
        ResponseKind::PanTiltPosition => {
            if payload.len() == 8 {
                match Nibbles::<8>::try_from(payload) {
                    Ok(nibbles) => {
                        // Extract as u16 values first (camera coordinates)
                        let pan_u16 = nibbles.u16_quad(0);
                        let tilt_u16 = nibbles.u16_quad(4);

                        // Convert from camera coordinates to logical coordinates using profile's coordinate system
                        let (pan, tilt) =
                            P::COORDINATE_SYSTEM.convert_from_camera_coords(pan_u16, tilt_u16);

                        Some(Ok(Response::Inquiry(InquiryResponse::PanTiltPosition {
                            pan,
                            tilt,
                        })))
                    }
                    Err(e) => Some(Err(e)),
                }
            } else if payload.len() == 4 {
                tracing::warn!(
                    "PanTiltPosition: Received compact format (4 bytes). Payload: {payload:02X?}. Treating as home position."
                );
                // For compact format, extract as u16 and convert
                let pan_u16 = if payload.len() >= 2 {
                    ((payload.as_slice()[0] as u16) << 8) | (payload.as_slice()[1] as u16)
                } else {
                    0x8000 // Center position for unsigned-centered systems
                };
                let tilt_u16 = if payload.len() >= 4 {
                    ((payload.as_slice()[2] as u16) << 8) | (payload.as_slice()[3] as u16)
                } else {
                    0x8000 // Center position for unsigned-centered systems
                };

                // Convert from camera coordinates to logical coordinates
                let (pan, tilt) =
                    P::COORDINATE_SYSTEM.convert_from_camera_coords(pan_u16, tilt_u16);

                Some(Ok(Response::Inquiry(InquiryResponse::PanTiltPosition {
                    pan,
                    tilt,
                })))
            } else {
                tracing::error!(
                    "PanTiltPosition: Invalid response length. Expected 8 or 4 bytes, got {len}. Payload: {payload:02X?}",
                    len = payload.len()
                );
                Some(Err(Error::InvalidResponseLength))
            }
        }
        _ => None,
    }
}
