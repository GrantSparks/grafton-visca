//! Motion observation values shared by the async, blocking, and dynamic views.

use std::time::Duration;

use crate::AffectedAxes;

/// Per-axis tolerance used while comparing two position snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
///
/// The type is `#[non_exhaustive]`: build it with [`MotionQuery::new`],
/// [`Default`] or [`From<AffectedAxes>`], then the `with_*` methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MotionQuery {
    /// Axes to sample. Unselected axes are never queried.
    pub axes: AffectedAxes,
    /// Maximum stable delta for each selected axis.
    pub tolerance: MovementTolerance,
    /// Minimum owner-clock time between the end of the first snapshot and the
    /// start of the second. Must be greater than zero.
    pub window: Duration,
}

impl MotionQuery {
    /// Default observation window, matching [`IdleWait`]'s default interval.
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
/// The type is `#[non_exhaustive]`: build it with [`IdleWait::new`], the
/// `for_*` presets, [`Default`] or [`From<Duration>`], then the `with_*`
/// methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct IdleWait {
    /// Axes to sample. Unselected axes are never queried.
    pub axes: AffectedAxes,
    /// Absolute observation budget.
    pub timeout: Duration,
    /// Maximum stable delta for each selected axis.
    pub tolerance: MovementTolerance,
    /// Minimum interval between complete snapshots.
    pub interval: Duration,
}

impl IdleWait {
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
    /// so this budgets 60 seconds over [`AffectedAxes::MOVEMENT`].
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

/// Raw pan/tilt position returned by the VISCA inquiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltPosition {
    /// Pan position in raw VISCA units.
    pub pan: i32,
    /// Tilt position in raw VISCA units.
    pub tilt: i32,
}

impl PanTiltPosition {
    /// Creates a raw pan/tilt position.
    pub const fn new(pan: i32, tilt: i32) -> Self {
        Self { pan, tilt }
    }

    /// Converts a position in the library's standard VISCA degree ranges.
    ///
    /// This conversion is only meaningful for the standard `i16` pan/tilt
    /// coordinate ranges. An out-of-range axis returns `NaN` rather than
    /// silently treating a profile-specific coordinate as the center position.
    /// Use [`Self::as_degrees_with_profile`] for a typed camera inquiry.
    #[must_use]
    pub fn as_degrees(&self) -> (f64, f64) {
        use crate::types::{PanPosition, TiltPosition};

        let pan = i16::try_from(self.pan)
            .ok()
            .and_then(|value| PanPosition::new(value).ok())
            .map_or(f64::NAN, |value| f64::from(value.to_degrees()));
        let tilt = i16::try_from(self.tilt)
            .ok()
            .and_then(|value| TiltPosition::new(value).ok())
            .map_or(f64::NAN, |value| f64::from(value.to_degrees()));
        (pan, tilt)
    }

    /// Converts the position using a typed camera profile's signed,
    /// profile-specified units per degree.
    ///
    /// This preserves profile-specific coordinates such as Sony BRC-300's
    /// signed 20-bit pan position returned by a typed inquiry, including a
    /// profile whose raw axis polarity is reverse to the library convention.
    #[must_use]
    pub fn as_degrees_with_profile<P: crate::capabilities::Profile>(
        &self,
        _profile: &P,
    ) -> (f64, f64) {
        (
            f64::from(self.pan) / f64::from(P::PAN_DEGREES_TO_UNITS),
            f64::from(self.tilt) / f64::from(P::TILT_DEGREES_TO_UNITS),
        )
    }

    /// Returns the raw `(pan, tilt)` values.
    #[must_use]
    pub const fn raw_values(&self) -> (i32, i32) {
        (self.pan, self.tilt)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{IdleWait, MotionQuery, MovementTolerance, PanTiltPosition};
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
    fn pan_tilt_position_profile_conversion_preserves_brc300_axis_polarity() {
        use crate::profiles::SonyBRC300;

        let left_up = PanTiltPosition::new(0x08A58, 0x493D);
        let (pan, tilt) = left_up.as_degrees_with_profile(&SonyBRC300);
        assert!(pan < 0.0);
        assert!(tilt < 0.0);
        assert!((pan + f64::from(0x08A58) / 208.0).abs() < f64::EPSILON);
        assert!((tilt + f64::from(0x493D) / 208.0).abs() < f64::EPSILON);

        let right_down = PanTiltPosition::new(-0x08A58, -0x186A);
        let (pan, tilt) = right_down.as_degrees_with_profile(&SonyBRC300);
        assert!(pan > 0.0);
        assert!(tilt > 0.0);
        assert!((pan - f64::from(0x08A58) / 208.0).abs() < f64::EPSILON);
        assert!((tilt - f64::from(0x186A) / 208.0).abs() < f64::EPSILON);

        let (standard_pan, standard_tilt) = left_up.as_degrees();
        assert!(standard_pan.is_nan());
        assert!(standard_tilt.is_nan());
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
    fn motion_query_default_and_conversion_select_movement_axes() {
        assert_eq!(MotionQuery::default().axes, AffectedAxes::MOVEMENT);
        assert_eq!(
            MotionQuery::from(AffectedAxes::FOCUS),
            MotionQuery::new(AffectedAxes::FOCUS)
        );
    }
}
