//! Drop-time STOP submission for in-flight movement operations.
//!
//! A public operation handle is linear: every terminal method consumes it. A
//! handle that instead disappears — an early `?` return, a `panic!` unwinding
//! past it, or a forgotten binding — is the one case where the caller never
//! told the owner what to do with physical motion the camera is still
//! performing. Leaving the hardware moving there is a physical-safety defect,
//! so dropping an unobserved movement handle enqueues the typed STOP for the
//! axes that operation affects.
//!
//! The classification is the branch's existing one, not a new taxonomy:
//!
//! * only an operation reaches this module at all, because
//!   [`AffectedAxes`] is non-empty by construction and the plain/inquiry
//!   request classes never produce a handle;
//! * an axis is stoppable when it has a typed STOP request — pan/tilt, zoom,
//!   and focus. Iris and ND-filter actuation has no STOP in the protocol;
//! * [`ControlClass::Urgent`] is exactly the closed set of stop and cancel
//!   requests, so a dropped STOP never re-sends itself.
//!
//! Preparation is deliberately lazy. Building the stop requests eagerly would
//! charge every submission for a path only an abandoned handle takes, so the
//! plan retains the profile facts and lowers them once, on drop.

#![allow(dead_code)] // Consumed by the async and blocking lifecycle handles.

use std::borrow::Borrow;

use crate::{
    completion,
    prepared::{prepare_operation, PreparedOperation},
    profile::{OperationalTuning, ProfileSpec},
    request::builtin::{FocusStop, PanTiltStop, ZoomStop},
    runtime::engine::RuntimeRequest,
    types::{PanSpeed, TiltSpeed},
    AffectedAxes, AffectedAxis, CameraId, ControlClass, Result,
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

/// Returns whether this axis has a typed STOP request.
const fn is_stoppable(axis: AffectedAxis) -> bool {
    matches!(
        axis,
        AffectedAxis::PanTilt | AffectedAxis::Zoom | AffectedAxis::Focus
    )
}

/// Returns whether dropping an operation with these classes must stop motion.
///
/// `ControlClass::Urgent` is the closed set of stop and cancel requests, so an
/// abandoned STOP is never answered with another STOP.
fn stops_on_drop(control: ControlClass, axes: AffectedAxes) -> bool {
    !matches!(control, ControlClass::Urgent) && axes.iter().any(is_stoppable)
}

/// The profile facts one admitted movement operation needs to lower its own
/// STOP if its handle is dropped unobserved.
///
/// `P` is the facade's profile ownership: an `Arc<ProfileSpec>` for the async
/// session, a `&'session ProfileSpec` for the borrowed blocking session.
#[derive(Clone)]
pub(crate) struct DropStopPlan<P> {
    axes: AffectedAxes,
    target: CameraId,
    profile: P,
    tuning: OperationalTuning,
}

impl<P> std::fmt::Debug for DropStopPlan<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DropStopPlan")
            .field("axes", &self.axes)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl<P> DropStopPlan<P>
where
    P: Borrow<ProfileSpec>,
{
    /// Records the drop-time stop plan for an admitted operation, or `None`
    /// when this operation is not movement that a STOP can end.
    pub(crate) fn new(
        control: ControlClass,
        axes: AffectedAxes,
        target: CameraId,
        profile: P,
        tuning: OperationalTuning,
    ) -> Option<Self> {
        stops_on_drop(control, axes).then_some(Self {
            axes,
            target,
            profile,
            tuning,
        })
    }

    /// Lowers one typed STOP per stoppable affected axis and hands each to the
    /// facade's best-effort submission seam, in canonical axis order.
    ///
    /// This runs inside `Drop`, including during panic unwinding, so it must
    /// never panic. Every lowering failure — an axis the profile does not
    /// support, a speed range that cannot encode a stop — is discarded rather
    /// than reported: the remaining axes still get their STOP.
    pub(crate) fn lower_each(&self, mut submit: impl FnMut(RuntimeRequest)) {
        let profile = self.profile.borrow();
        for axis in self.axes.iter() {
            let prepared = match axis {
                AffectedAxis::PanTilt => pan_tilt_stop_request(profile)
                    .and_then(|stop| self.prepare(&stop, profile))
                    .ok(),
                AffectedAxis::Zoom => self.prepare(&ZoomStop, profile).ok(),
                AffectedAxis::Focus => self.prepare(&FocusStop, profile).ok(),
                AffectedAxis::Iris | AffectedAxis::NdFilter => None,
            };
            if let Some(request) = prepared {
                submit(request);
            }
        }
    }

    fn prepare<O>(&self, stop: &O, profile: &ProfileSpec) -> Result<RuntimeRequest>
    where
        O: crate::OperationCommand<completion::AppliedOnly> + ?Sized,
    {
        prepare_operation::<completion::AppliedOnly, _>(stop, self.target, profile, self.tuning)
            .map(PreparedOperation::into_stop_request)
    }
}
