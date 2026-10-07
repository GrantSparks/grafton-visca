//! Public terminal cancellation outcomes.

/// The conclusive result of observing a cancellation request.
///
/// Cancellation can lose a race with the original operation's successful
/// completion. That exact race is reported as [`Self::Completed`], never as a
/// synthetic cancellation success.
///
/// The enum is `#[non_exhaustive]` so later releases can report further
/// honest outcomes; match it with a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum CancellationOutcome {
    /// The original operation was conclusively cancelled.
    Cancelled,
    /// The original operation completed successfully before cancellation won.
    Completed,
}

/// Evidence establishing a targeted operation's settlement.
/// Polled stability does not establish arrival at the requested endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Settlement {
    /// The profile declares this operation's protocol completion exact.
    #[non_exhaustive]
    ProfileCompletion {
        /// The operation's axes.
        axes: crate::AffectedAxes,
    },
    /// The selected positions were stable across this sampling window.
    #[non_exhaustive]
    ObservedStable {
        /// The sampled axes.
        axes: crate::AffectedAxes,
        /// Minimum time from the first snapshot's end to the second's start.
        window: std::time::Duration,
        /// Maximum stable position delta for each axis.
        tolerance: crate::camera::MovementTolerance,
    },
}

/// The result of one independently dispatched STOP in an owner halt.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum HaltOutcome {
    /// The profile has no typed STOP for this axis.
    Unsupported,
    /// This axis's STOP was applied; this is protocol evidence, not physical feedback.
    Applied,
    /// Preparation, admission, protocol execution, or observation failed.
    Failed(crate::Error),
}

/// Results of one owner-ordered halt under one end-to-end deadline.
/// Older declared queued motion is superseded; later submissions run normally.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[must_use = "inspect each supported axis result"]
pub struct HaltReport {
    /// Pan/tilt STOP result.
    pub pan_tilt: HaltOutcome,
    /// Zoom STOP result.
    pub zoom: HaltOutcome,
    /// Focus STOP result.
    pub focus: HaltOutcome,
}

impl Settlement {
    /// Constructs evidence from a profile-declared operation completion.
    #[must_use]
    pub const fn profile_completion(axes: crate::AffectedAxes) -> Self {
        Self::ProfileCompletion { axes }
    }
    /// Constructs a stable-sample report. This describes evidence, not an endpoint guarantee.
    #[must_use]
    pub const fn observed_stable(
        axes: crate::AffectedAxes,
        window: std::time::Duration,
        tolerance: crate::camera::MovementTolerance,
    ) -> Self {
        Self::ObservedStable {
            axes,
            window,
            tolerance,
        }
    }
}
impl HaltReport {
    /// Constructs a report from the three independent axis outcomes.
    pub const fn new(pan_tilt: HaltOutcome, zoom: HaltOutcome, focus: HaltOutcome) -> Self {
        Self {
            pan_tilt,
            zoom,
            focus,
        }
    }
    /// Explicitly collapses the report to its first supported-axis failure.
    /// Unsupported axes are skipped. Inspect the fields to retain every outcome.
    pub fn into_result(self) -> Result<(), crate::Error> {
        for result in [self.pan_tilt, self.zoom, self.focus] {
            match result {
                HaltOutcome::Failed(error) => return Err(error),
                HaltOutcome::Unsupported | HaltOutcome::Applied => {}
            }
        }
        Ok(())
    }
}
