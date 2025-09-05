//! Preset capability trait and associated types.

// Standard library
use std::{ops::Range, time::Duration};

// Local modules
use crate::capabilities::ValidationError;

/// Trait for cameras that support preset positions.
///
/// This trait defines the constants and capabilities for preset management
/// including storing, recalling, and touring preset positions.
pub trait Presets {
    /// Maximum number of presets supported (excluding home position).
    const MAX_PRESETS: u8;

    /// Valid range for preset movement speed.
    const PRESET_SPEED_RANGE: Range<u8>;

    /// Whether camera supports preset tour functionality.
    const SUPPORTS_PRESET_TOUR: bool;

    /// Time needed after preset recall before camera is ready.
    /// Some cameras need stabilization time after moving to preset.
    const PRESET_RECALL_DELAY: Duration = Duration::from_millis(0);

    /// Whether camera supports preset thumbnail capture.
    const SUPPORTS_PRESET_THUMBNAIL: bool = false;

    /// Whether camera supports preset names/labels.
    const SUPPORTS_PRESET_NAMES: bool = false;

    /// Maximum preset name length if supported.
    const MAX_PRESET_NAME_LENGTH: usize = 0;
}

/// Extension trait that adds validation methods to cameras with preset support.
pub trait PresetsExt: Presets {
    /// Validate preset number is within range.
    /// Note: Preset 0 is typically the home position.
    fn validate_preset_number(&self, preset: u8) -> Result<u8, ValidationError> {
        if preset <= Self::MAX_PRESETS {
            Ok(preset)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "preset number",
                value: preset as f64,
                min: 0.0,
                max: Self::MAX_PRESETS as f64,
            })
        }
    }

    /// Validate preset recall speed.
    fn validate_preset_speed(&self, speed: u8) -> Result<u8, ValidationError> {
        if Self::PRESET_SPEED_RANGE.contains(&speed) {
            Ok(speed)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "preset speed",
                value: speed as f64,
                min: Self::PRESET_SPEED_RANGE.start as f64,
                max: (Self::PRESET_SPEED_RANGE.end - 1) as f64,
            })
        }
    }

    /// Check if preset tour is supported.
    fn can_tour_presets(&self) -> bool {
        Self::SUPPORTS_PRESET_TOUR
    }

    /// Get the delay needed after preset recall.
    fn preset_recall_delay(&self) -> Duration {
        Self::PRESET_RECALL_DELAY
    }

    /// Check if preset is the home position.
    fn is_home_preset(&self, preset: u8) -> bool {
        preset == 0
    }
}

// Automatic implementation for all types that support presets
impl<T: Presets> PresetsExt for T {}

/// Preset tour configuration for cameras that support it.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetTour {
    /// List of preset numbers to visit in order.
    pub presets: Vec<u8>,
    /// Time to stay at each preset in seconds.
    pub dwell_time: Duration,
    /// Speed to move between presets.
    pub movement_speed: u8,
    /// Whether to loop the tour.
    pub loop_tour: bool,
}

impl PresetTour {
    /// Create a new preset tour configuration.
    pub fn new(presets: Vec<u8>) -> Self {
        Self {
            presets,
            dwell_time: Duration::from_secs(5),
            movement_speed: 10,
            loop_tour: true,
        }
    }

    /// Set the dwell time at each preset.
    pub fn with_dwell_time(mut self, duration: Duration) -> Self {
        self.dwell_time = duration;
        self
    }

    /// Set the movement speed between presets.
    pub fn with_speed(mut self, speed: u8) -> Self {
        self.movement_speed = speed;
        self
    }

    /// Set whether to loop the tour.
    pub fn with_loop(mut self, should_loop: bool) -> Self {
        self.loop_tour = should_loop;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCamera;

    impl Presets for TestCamera {
        const MAX_PRESETS: u8 = 89;
        const PRESET_SPEED_RANGE: Range<u8> = 1..25;
        const SUPPORTS_PRESET_TOUR: bool = true;
        const PRESET_RECALL_DELAY: Duration = Duration::from_millis(100);
    }

    #[test]
    fn test_preset_validation() {
        let camera = TestCamera;

        assert!(camera.validate_preset_number(0).is_ok());
        assert!(camera.validate_preset_number(89).is_ok());
        assert!(camera.validate_preset_number(90).is_err());
    }

    #[test]
    fn test_preset_speed_validation() {
        let camera = TestCamera;

        assert!(camera.validate_preset_speed(1).is_ok());
        assert!(camera.validate_preset_speed(24).is_ok());
        assert!(camera.validate_preset_speed(0).is_err());
        assert!(camera.validate_preset_speed(25).is_err());
    }

    #[test]
    fn test_home_preset() {
        let camera = TestCamera;

        assert!(camera.is_home_preset(0));
        assert!(!camera.is_home_preset(1));
    }

    #[test]
    fn test_preset_tour() {
        let tour = PresetTour::new(vec![1, 2, 3])
            .with_dwell_time(Duration::from_secs(10))
            .with_speed(15)
            .with_loop(false);

        assert_eq!(tour.presets, vec![1, 2, 3]);
        assert_eq!(tour.dwell_time, Duration::from_secs(10));
        assert_eq!(tour.movement_speed, 15);
        assert!(!tour.loop_tour);
    }
}
