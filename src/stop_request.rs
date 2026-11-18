//! Profile-aware construction of the typed pan/tilt STOP request.
//!
//! The pan/tilt `stop()` noun and halt preparation share this profile-aware
//! speed selection.

use crate::{
    command::semantics::StopCommand,
    profiles::ProfileSpec,
    request::builtin::PanTiltStop,
    types::{PanSpeed, TiltSpeed},
    Result,
};

impl StopCommand {
    /// All STOPs are installation independent. Their value checks use the
    /// same `BuiltinValidation` implementation as target preparation.
    pub(crate) fn validate(self, profile: &ProfileSpec) -> Result<()> {
        use crate::request::builtin::{BuiltinValidation, FocusStop, ZoomStop};
        crate::request::builtin::validate_row(self.row(), profile)?;
        match self {
            Self::PanTiltStop => pan_tilt_stop_request(profile).map(|_| ()),
            Self::ZoomStop => ZoomStop.validate(profile),
            Self::FocusStop => FocusStop.validate(profile),
            Self::Fr7ZoomHighResolutionStop => {
                crate::command::Fr7ZoomHighResolutionStop.validate(profile)
            }
        }
    }
}

/// One admissible STOP per independent axis set, in halt report order.
#[derive(Debug, Clone, Copy, Default)]
#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) struct StopPaths {
    pub(crate) pan_tilt: Option<StopCommand>,
    pub(crate) zoom: Option<StopCommand>,
    pub(crate) focus: Option<StopCommand>,
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl StopPaths {
    pub(crate) const fn rows(self) -> [Option<StopCommand>; 3] {
        [self.pan_tilt, self.zoom, self.focus]
    }

    pub(crate) fn count(self) -> u8 {
        u8::from(self.pan_tilt.is_some())
            + u8::from(self.zoom.is_some())
            + u8::from(self.focus.is_some())
    }
}

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
    let capabilities =
        profile
            .capabilities()
            .pan_tilt
            .as_ref()
            .ok_or(crate::Error::FeatureNotSupported {
                feature: "pan/tilt",
            })?;
    let pan_value = if capabilities.pan_speed.contains(PREFERRED_PAN_STOP_SPEED) {
        PREFERRED_PAN_STOP_SPEED
    } else {
        capabilities.pan_speed.min()
    };
    let tilt_value = if capabilities.tilt_speed.contains(PREFERRED_TILT_STOP_SPEED) {
        PREFERRED_TILT_STOP_SPEED
    } else {
        capabilities.tilt_speed.min()
    };
    Ok(PanTiltStop::new(
        PanSpeed::new(pan_value)?,
        TiltSpeed::new(tilt_value)?,
    ))
}
