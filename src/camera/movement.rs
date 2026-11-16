//! Motion observation values shared by the async, blocking, and dynamic views.

use std::time::Duration;

use crate::AffectedAxes;

/// Per-axis tolerance used while comparing two position snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub struct MovementTolerance {
    /// Pan/tilt tolerance in raw VISCA units.
    pub pan_tilt: i32,
    /// Zoom tolerance in raw VISCA units.
    pub zoom: u16,
    /// Focus tolerance in raw VISCA units.
    pub focus: u16,
    /// Iris tolerance in raw VISCA units.
    pub iris: u16,
    /// ND-filter tolerance in raw VISCA units.
    pub nd_filter: u16,
}

impl Default for MovementTolerance {
    fn default() -> Self {
        Self {
            pan_tilt: 2,
            zoom: 10,
            focus: 5,
            iris: 0,
            nd_filter: 0,
        }
    }
}

/// Exact axes, tolerance, and observation window for one movement observation.
///
/// `is_moving` reads one complete position snapshot, waits until at least
/// [`window`](Self::window) has elapsed on the owner clock since that snapshot
/// was received, and then reads a second snapshot. It reports movement when
/// any selected axis changed by more than its tolerance between the two.
///
/// `false` therefore means *no movement was detected over at least `window`*.
/// It is evidence about two separated samples, not proof that the camera is
/// physically at rest: an axis that moves slowly enough to stay within
/// tolerance over the window, or that returns to its starting position within
/// it, is not detected. A longer window detects slower movement but makes the
/// call take longer. Profile inquiry pacing can only lengthen the effective
/// window, never shorten it.
/// The overall observation budget covers this window plus two complete
/// snapshots, using each prepared inquiry's observer timeout (including its
/// retry budget) and profile-selected inquiry pacing.
///
/// The type is `#[non_exhaustive]`: build it with [`crate::types::MotionQuery::new`],
/// [`Default`] or [`From<AffectedAxes>`], then the `with_*` methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub struct MotionQuery {
    /// Axes to sample. Unselected axes are never queried.
    pub axes: AffectedAxes,
    /// Maximum stable delta for each selected axis.
    pub tolerance: MovementTolerance,
    /// Minimum owner-clock time between the end of the first snapshot and the
    /// start of the second. Must be greater than zero.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    pub window: Duration,
}

impl MotionQuery {
    /// Default observation window, matching [`crate::types::IdleWait`]'s default interval.
    pub const DEFAULT_WINDOW: Duration = Duration::from_millis(100);

    /// Creates a query with the default tolerance and observation window.
    #[must_use]
    pub const fn new(axes: AffectedAxes) -> Self {
        Self {
            axes,
            tolerance: MovementTolerance {
                pan_tilt: 2,
                zoom: 10,
                focus: 5,
                iris: 0,
                nd_filter: 0,
            },
            window: Self::DEFAULT_WINDOW,
        }
    }

    /// Replaces the movement tolerance.
    #[must_use]
    pub const fn with_tolerance(mut self, tolerance: MovementTolerance) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Replaces the observation window.
    ///
    /// A zero window is rejected with [`Error::InvalidParameter`] before any
    /// inquiry is sent: two back-to-back samples cannot establish movement
    /// evidence over a positive duration.
    ///
    /// [`Error::InvalidParameter`]: crate::Error::InvalidParameter
    #[must_use]
    pub const fn with_window(mut self, window: Duration) -> Self {
        self.window = window;
        self
    }
}

impl Default for MotionQuery {
    /// Samples every mechanical movement axis with the default tolerance and
    /// observation window.
    fn default() -> Self {
        Self::new(AffectedAxes::MOVEMENT)
    }
}

impl From<AffectedAxes> for MotionQuery {
    /// Samples `axes` with the default tolerance and observation window.
    fn from(axes: AffectedAxes) -> Self {
        Self::new(axes)
    }
}

/// Complete policy for waiting until selected axes become idle.
///
/// The type is `#[non_exhaustive]`: build it with [`crate::types::IdleWait::new`], the
/// `for_*` presets, [`Default`] or [`From<Duration>`], then the `with_*`
/// methods. The sampling interval must be positive; zero is refused before
/// I/O and when deserializing a policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema), schemars(!try_from))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(try_from = "IdleWaitData"))]
#[non_exhaustive]
pub struct IdleWait {
    /// Axes to sample. Unselected axes are never queried.
    axes: AffectedAxes,
    /// Absolute observation budget. It bounds every sample, the first
    /// included: a wait whose budget elapses before its first inquiry is
    /// admitted sends nothing and fails with the same observation timeout.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    timeout: Duration,
    /// Maximum stable delta for each selected axis.
    tolerance: MovementTolerance,
    /// Minimum interval between complete snapshots.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    interval: Duration,
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct IdleWaitData {
    axes: AffectedAxes,
    timeout: Duration,
    tolerance: MovementTolerance,
    interval: Duration,
}

#[cfg(feature = "serde")]
impl TryFrom<IdleWaitData> for IdleWait {
    type Error = crate::Error;

    fn try_from(data: IdleWaitData) -> crate::Result<Self> {
        let wait = Self {
            axes: data.axes,
            timeout: data.timeout,
            tolerance: data.tolerance,
            interval: data.interval,
        };
        wait.validate()?;
        Ok(wait)
    }
}

impl IdleWait {
    /// The selected axes; unselected axes are never queried.
    #[must_use]
    pub const fn axes(&self) -> AffectedAxes {
        self.axes
    }

    /// The absolute observation budget, including the first sample.
    #[must_use]
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    /// The maximum stable delta for each selected axis.
    #[must_use]
    pub const fn tolerance(&self) -> MovementTolerance {
        self.tolerance
    }

    /// The minimum interval between complete snapshots.
    #[must_use]
    pub const fn interval(&self) -> Duration {
        self.interval
    }

    #[cfg(any(feature = "blocking", feature = "async", feature = "serde", test))]
    pub(crate) fn validate(&self) -> crate::Result<()> {
        if self.interval.is_zero() {
            return Err(crate::Error::InvalidParameter {
                parameter: "idle sampling interval",
                value: "0ns".into(),
                reason: "position snapshots require a positive sampling interval".into(),
            });
        }
        Ok(())
    }

    /// Creates an idle wait with default tolerance and a 100ms interval.
    #[must_use]
    pub const fn new(axes: AffectedAxes, timeout: Duration) -> Self {
        Self {
            axes,
            timeout,
            tolerance: MovementTolerance {
                pan_tilt: 2,
                zoom: 10,
                focus: 5,
                iris: 0,
                nd_filter: 0,
            },
            interval: Duration::from_millis(100),
        }
    }

    /// Replaces the movement tolerance.
    #[must_use]
    pub const fn with_tolerance(mut self, tolerance: MovementTolerance) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Replaces the interval between snapshots.
    ///
    /// A zero interval is rejected with [`crate::Error::InvalidParameter`]
    /// before any inquiry is sent. Two back-to-back snapshots cannot establish
    /// idle evidence over a positive duration. Deserialization enforces the
    /// same rule.
    #[must_use]
    pub const fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Replaces the observed axes.
    #[must_use]
    pub const fn with_axes(mut self, axes: AffectedAxes) -> Self {
        self.axes = axes;
        self
    }

    /// Replaces the absolute observation budget.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Waits for a preset recall over every mechanical movement axis.
    ///
    /// A preset moves pan/tilt, zoom, and focus together and can take a while,
    /// so this budgets 60 seconds over [`crate::types::AffectedAxes::MOVEMENT`].
    #[must_use]
    pub const fn for_preset_recall() -> Self {
        Self::new(AffectedAxes::MOVEMENT, Duration::from_secs(60))
    }

    /// Waits for pan/tilt movement only, budgeting 30 seconds.
    #[must_use]
    pub const fn for_pan_tilt() -> Self {
        Self::new(AffectedAxes::PAN_TILT, Duration::from_secs(30))
    }

    /// Waits for zoom movement only, budgeting 15 seconds.
    #[must_use]
    pub const fn for_zoom() -> Self {
        Self::new(AffectedAxes::ZOOM, Duration::from_secs(15))
    }

    /// Waits for focus movement only, budgeting 10 seconds.
    #[must_use]
    pub const fn for_focus() -> Self {
        Self::new(AffectedAxes::FOCUS, Duration::from_secs(10))
    }
}

impl Default for IdleWait {
    /// Waits up to 30 seconds over every mechanical movement axis.
    fn default() -> Self {
        Self::new(AffectedAxes::MOVEMENT, Duration::from_secs(30))
    }
}

impl From<Duration> for IdleWait {
    /// Waits up to `timeout` over every mechanical movement axis.
    fn from(timeout: Duration) -> Self {
        Self::new(AffectedAxes::MOVEMENT, timeout)
    }
}

/// Camera-independent pan/tilt position in degrees.
///
/// Positive pan is right and positive tilt is up. The same value represents an
/// absolute position or a relative offset. Construction checks finiteness;
/// preparation applies the target's coordinate conversion and physical limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct PanTiltPosition {
    /// Pan angle in degrees; positive is right.
    pub pan: crate::units::Degrees,
    /// Tilt angle in degrees; positive is up.
    pub tilt: crate::units::Degrees,
}

impl PanTiltPosition {
    /// Creates a position or offset, rejecting non-finite angles.
    pub fn from_degrees(pan: f32, tilt: f32) -> crate::Result<Self> {
        Ok(Self {
            pan: crate::units::Degrees::new(pan)?,
            tilt: crate::units::Degrees::new(tilt)?,
        })
    }
}

/// Raw pan/tilt position returned by a low-level VISCA inquiry.
///
/// Units and axis polarity are profile-specific. Use the target profile's
/// coordinate conversion to obtain a physical [`crate::types::PanTiltPosition`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct PanTiltUnits {
    /// Pan position in raw VISCA units.
    pub pan: i32,
    /// Tilt position in raw VISCA units.
    pub tilt: i32,
}

impl PanTiltUnits {
    /// Creates a raw pan/tilt position.
    #[must_use]
    pub const fn new(pan: i32, tilt: i32) -> Self {
        Self { pan, tilt }
    }

    /// Returns the raw `(pan, tilt)` values.
    #[must_use]
    pub const fn raw_values(self) -> (i32, i32) {
        (self.pan, self.tilt)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{IdleWait, MotionQuery, MovementTolerance, PanTiltPosition, PanTiltUnits};
    use crate::AffectedAxes;

    #[test]
    fn named_wait_presets_select_their_axis_and_budget() {
        let expected = [
            (
                IdleWait::for_preset_recall(),
                AffectedAxes::MOVEMENT,
                Duration::from_secs(60),
            ),
            (
                IdleWait::for_pan_tilt(),
                AffectedAxes::PAN_TILT,
                Duration::from_secs(30),
            ),
            (
                IdleWait::for_zoom(),
                AffectedAxes::ZOOM,
                Duration::from_secs(15),
            ),
            (
                IdleWait::for_focus(),
                AffectedAxes::FOCUS,
                Duration::from_secs(10),
            ),
        ];

        for (wait, axes, timeout) in expected {
            assert_eq!(wait.axes, axes);
            assert_eq!(wait.timeout, timeout);
            assert_eq!(wait.tolerance, MovementTolerance::default());
            assert_eq!(wait.interval, Duration::from_millis(100));
        }
    }

    #[test]
    fn positions_separate_checked_degrees_from_raw_units() -> crate::Result<()> {
        let position = PanTiltPosition::from_degrees(45.0, -15.0)?;
        assert_eq!(position.pan.value(), 45.0);
        assert_eq!(position.tilt.value(), -15.0);
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(PanTiltPosition::from_degrees(invalid, 0.0).is_err());
            assert!(PanTiltPosition::from_degrees(0.0, invalid).is_err());
        }
        let raw = PanTiltUnits::new(-0x02490, 0x0C30);
        assert_eq!(raw.raw_values(), (-0x02490, 0x0C30));
        Ok(())
    }

    #[test]
    fn idle_wait_default_and_duration_conversion_watch_every_movement_axis() {
        let default = IdleWait::default();
        assert_eq!(default.axes, AffectedAxes::MOVEMENT);
        assert_eq!(default.timeout, Duration::from_secs(30));

        let converted = IdleWait::from(Duration::from_millis(2500));
        assert_eq!(converted.axes, AffectedAxes::MOVEMENT);
        assert_eq!(converted.timeout, Duration::from_millis(2500));
        assert_eq!(converted.interval, default.interval);
        assert_eq!(converted.tolerance, default.tolerance);

        let inferred: IdleWait = Duration::from_secs(1).into();
        assert_eq!(
            inferred,
            IdleWait::new(AffectedAxes::MOVEMENT, Duration::from_secs(1))
        );
    }

    #[test]
    fn idle_wait_builders_replace_axes_and_timeout() {
        let wait = IdleWait::default()
            .with_axes(AffectedAxes::ZOOM)
            .with_timeout(Duration::from_secs(3))
            .with_interval(Duration::from_millis(25));
        assert_eq!(wait.axes, AffectedAxes::ZOOM);
        assert_eq!(wait.timeout, Duration::from_secs(3));
        assert_eq!(wait.interval, Duration::from_millis(25));
    }

    #[test]
    fn idle_wait_requires_a_positive_sampling_interval() {
        let wait = IdleWait::for_zoom().with_interval(Duration::ZERO);
        assert_eq!(wait.axes(), AffectedAxes::ZOOM);
        assert_eq!(wait.timeout(), Duration::from_secs(15));
        assert_eq!(wait.tolerance(), MovementTolerance::default());
        assert_eq!(wait.interval(), Duration::ZERO);
        assert!(matches!(
            wait.validate(),
            Err(crate::Error::InvalidParameter {
                parameter: "idle sampling interval",
                ..
            })
        ));
        assert!(wait
            .with_interval(Duration::from_nanos(1))
            .validate()
            .is_ok());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn idle_wait_deserialization_uses_the_same_interval_validation() -> serde_json::Result<()> {
        let wait = IdleWait::for_zoom().with_interval(Duration::from_millis(25));
        let mut json = serde_json::to_value(wait)?;
        assert_eq!(serde_json::from_value::<IdleWait>(json.clone())?, wait);
        json["interval"] = serde_json::json!({"secs": 0, "nanos": 0});
        assert!(serde_json::from_value::<IdleWait>(json)
            .err()
            .is_some_and(|error| error.to_string().contains("idle sampling interval")));
        Ok(())
    }

    #[test]
    fn motion_query_default_and_conversion_select_movement_axes() {
        assert_eq!(MotionQuery::default().axes, AffectedAxes::MOVEMENT);
        assert_eq!(
            MotionQuery::from(AffectedAxes::FOCUS),
            MotionQuery::new(AffectedAxes::FOCUS)
        );
    }
}
