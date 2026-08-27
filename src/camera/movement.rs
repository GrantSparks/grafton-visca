//! Motion observation values shared by the async, blocking, and dynamic views.

use std::time::Duration;

use crate::AffectedAxes;

/// Per-axis tolerance used while comparing two position snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovementTolerance {
    /// Pan/tilt tolerance in raw VISCA units.
    pub pan_tilt: i16,
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

/// Exact axes and tolerance for one movement observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionQuery {
    /// Axes to sample. Unselected axes are never queried.
    pub axes: AffectedAxes,
    /// Maximum stable delta for each selected axis.
    pub tolerance: MovementTolerance,
}

impl MotionQuery {
    /// Creates a query with the default tolerance.
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
        }
    }

    /// Replaces the movement tolerance.
    #[must_use]
    pub const fn with_tolerance(mut self, tolerance: MovementTolerance) -> Self {
        self.tolerance = tolerance;
        self
    }
}

impl Default for MotionQuery {
    /// Samples every mechanical movement axis with the default tolerance.
    fn default() -> Self {
        Self::new(AffectedAxes::MOVEMENT)
    }
}

impl From<AffectedAxes> for MotionQuery {
    /// Samples `axes` with the default tolerance.
    fn from(axes: AffectedAxes) -> Self {
        Self::new(axes)
    }
}

/// Complete policy for waiting until selected axes become idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub pan: i16,
    /// Tilt position in raw VISCA units.
    pub tilt: i16,
}

impl PanTiltPosition {
    /// Creates a raw pan/tilt position.
    pub const fn new(pan: i16, tilt: i16) -> Self {
        Self { pan, tilt }
    }

    /// Converts the position to the library's standard degree ranges.
    #[must_use]
    pub fn as_degrees(&self) -> (f64, f64) {
        use crate::types::{PanPosition, TiltPosition};

        let pan = PanPosition::new(self.pan).unwrap_or(PanPosition::CENTER);
        let tilt = TiltPosition::new(self.tilt).unwrap_or(TiltPosition::CENTER);
        (f64::from(pan.to_degrees()), f64::from(tilt.to_degrees()))
    }

    /// Returns the raw `(pan, tilt)` values.
    #[must_use]
    pub const fn raw_values(&self) -> (i16, i16) {
        (self.pan, self.tilt)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{IdleWait, MotionQuery, MovementTolerance};
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
