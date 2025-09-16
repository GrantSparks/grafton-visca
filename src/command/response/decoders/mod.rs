//! Domain-specific decoders for VISCA inquiry responses.

mod color;
mod exposure;
mod focus;
mod image;
mod pan_tilt;
mod power;
mod system;
mod tally;
mod zoom;

use super::{
    payload::Payload,
    types::{ViscaResponse, ViscaResponseType},
};
use crate::{capabilities::Profile, error::Error};

/// Try each domain decoder until one claims the ViscaResponseType.
///
/// Each submodule exposes a decode function that returns Some(Result) if
/// the response type belongs to that domain, or None otherwise.
pub(crate) fn dispatch(
    kind: ViscaResponseType,
    payload: Payload<'_>,
) -> Result<ViscaResponse, Error> {
    // Try each decoder in order
    for decoder_fn in &[
        power::decode,
        pan_tilt::decode,
        zoom::decode,
        focus::decode,
        exposure::decode,
        color::decode,
        image::decode,
        system::decode,
        tally::decode,
    ] {
        if let Some(result) = decoder_fn(kind, payload) {
            return result;
        }
    }

    // If no decoder handled this type, return an error
    Err(Error::Unsupported)
}

/// Profile-aware decoder dispatch for responses that need coordinate conversion.
///
/// This function uses the Profile's COORDINATE_SYSTEM to correctly convert
/// pan/tilt values from camera coordinates to logical coordinates.
pub(crate) fn dispatch_for<P: Profile>(
    kind: ViscaResponseType,
    payload: Payload<'_>,
) -> Result<ViscaResponse, Error> {
    // Special handling for PanTiltPosition which needs coordinate conversion
    if kind == ViscaResponseType::PanTiltPosition {
        return pan_tilt::decode_for::<P>(kind, payload).ok_or(Error::Unsupported)?;
    }

    // For all other response types, use the standard dispatch
    dispatch(kind, payload)
}
