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
