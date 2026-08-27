//! Profile-aware construction of the typed pan/tilt STOP request.
//!
//! The blocking and async facades each expose a `stop_pan_tilt` entry point,
//! and both need the same profile-dependent speed selection to build a valid
//! stop frame. This module holds that one implementation so the two facades
//! cannot drift apart.

#![cfg(any(feature = "blocking", feature = "async"))]

use crate::{
    profile::ProfileSpec,
    request::builtin::PanTiltStop,
    types::{PanSpeed, TiltSpeed},
    Result,
};

/// The medium pan speed used for a stop frame when the profile allows it.
const PREFERRED_PAN_STOP_SPEED: u8 = 12;
/// The medium tilt speed used for a stop frame when the profile allows it.
const PREFERRED_TILT_STOP_SPEED: u8 = 10;

/// Builds the typed pan/tilt STOP request valid for this exact profile.
///
/// The stop direction carries protocol speed fields even though it does not
/// move. The established medium encoding is kept when the profile permits it
/// and narrower runtime ranges fall back to their first valid speed.
///
/// # Errors
///
/// Returns an error when the profile's declared speed range cannot produce a
/// valid speed field.
pub(crate) fn pan_tilt_stop_request(profile: &ProfileSpec) -> Result<PanTiltStop> {
    let capabilities = profile.capabilities();
    let pan_value = if capabilities.pan_speed.contains(&PREFERRED_PAN_STOP_SPEED) {
        PREFERRED_PAN_STOP_SPEED
    } else {
        *capabilities.pan_speed.start()
    };
    let tilt_value = if capabilities.tilt_speed.contains(&PREFERRED_TILT_STOP_SPEED) {
        PREFERRED_TILT_STOP_SPEED
    } else {
        *capabilities.tilt_speed.start()
    };
    Ok(PanTiltStop::new(
        PanSpeed::new(pan_value)?,
        TiltSpeed::new(tilt_value)?,
    ))
}
