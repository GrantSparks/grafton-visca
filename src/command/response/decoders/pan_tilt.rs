//! Pan/Tilt-related response decoders.
//!
//! # VISCA Pan/Tilt Position Specification
//!
//! Per the VISCA specification, pan/tilt position inquiries return **two 16-bit values**
//! encoded as 8 nibbles (bytes with values 0x00-0x0F):
//! - Response format: `90 50 0w 0x 0y 0z 0p 0q 0r 0s FF`
//! - Pan position: `(w << 12) | (x << 8) | (y << 4) | z`
//! - Tilt position: `(p << 12) | (q << 8) | (r << 4) | s`
//!
//! ## Coordinate System Encoding
//!
//! Different camera models use different coordinate encodings:
//! - **Signed (two's complement)**: Values like `0xFFFF` represent negative positions
//! - **Unsigned centered**: Values centered around `0x8000` (e.g., `0x8000` = center)
//!
//! This library provides profile-aware decoding via `CoordinateSystem` to abstract
//! these differences. The `decode_for<P: Profile>()` function automatically converts
//! camera-native coordinates to logical signed i16 values based on the profile.

use crate::{
    capabilities::{PanTilt, Profile},
    command::{
        response::{
            payload::{Nibbles, Payload},
            types::{InquiryKind, Response},
        },
        InquiryData,
    },
    error::Error,
};

/// Decode pan/tilt-related inquiry responses without profile awareness.
///
/// This decoder interprets pan/tilt positions as signed 16-bit values.
/// For profile-aware decoding that respects camera-specific coordinate systems,
/// use `decode_for<P>()` instead.
pub(crate) fn decode(kind: InquiryKind, payload: Payload<'_>) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::PanTiltPosition => {
            if payload.len() == 8 {
                match Nibbles::<8>::try_from(payload) {
                    Ok(nibbles) => {
                        let pan = nibbles.i16_quad(0);
                        let tilt = nibbles.i16_quad(4);
                        Some(Ok(Response::Inquiry(InquiryData::PanTiltPosition {
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
                Some(Ok(Response::Inquiry(InquiryData::PanTiltPosition {
                    pan,
                    tilt,
                })))
            } else {
                // Return None to indicate this decoder doesn't handle this payload
                // This prevents wrong inquiry matching when 1-byte responses arrive
                tracing::debug!(
                    "PanTiltPosition: Payload length {len} doesn't match pan/tilt format (expected 8 or 4 bytes)",
                    len = payload.len()
                );
                None
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
    kind: InquiryKind,
    payload: Payload<'_>,
) -> Option<Result<Response, Error>> {
    match kind {
        InquiryKind::PanTiltPosition => {
            if payload.len() == 8 {
                match Nibbles::<8>::try_from(payload) {
                    Ok(nibbles) => {
                        // Extract as u16 values first (camera coordinates)
                        let pan_u16 = nibbles.u16_quad(0);
                        let tilt_u16 = nibbles.u16_quad(4);

                        // Convert from camera coordinates to logical coordinates using profile's coordinate system
                        let (pan, tilt) =
                            P::COORDINATE_SYSTEM.convert_from_camera_coords(pan_u16, tilt_u16);

                        Some(Ok(Response::Inquiry(InquiryData::PanTiltPosition {
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

                Some(Ok(Response::Inquiry(InquiryData::PanTiltPosition {
                    pan,
                    tilt,
                })))
            } else {
                // Return None to indicate this decoder doesn't handle this payload
                // This prevents wrong inquiry matching when 1-byte responses arrive
                tracing::debug!(
                    "PanTiltPosition: Payload length {len} doesn't match pan/tilt format (expected 8 or 4 bytes)",
                    len = payload.len()
                );
                None
            }
        }
        _ => None,
    }
}
