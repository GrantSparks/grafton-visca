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

use super::types::{ViscaResponse, ViscaResponseType};
use crate::error::Error;

/// Try each domain decoder until one claims the ViscaResponseType.
///
/// Each submodule exposes a decode function that returns Some(Result) if
/// the response type belongs to that domain, or None otherwise.
pub(crate) fn dispatch(kind: ViscaResponseType, payload: &[u8]) -> Result<ViscaResponse, Error> {
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
