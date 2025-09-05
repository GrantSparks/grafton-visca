//! Pan/Tilt capability trait and associated types.

use std::{ops::Range, time::Duration};

use crate::capabilities::{CoordinateSystem, ValidationError};

/// Trait for cameras that support pan and tilt movement.
///
/// This trait defines the constants and capabilities for pan/tilt operations.
/// Cameras implementing this trait gain access to pan/tilt movement methods.
pub trait PanTilt {
    /// Valid range for pan position in VISCA units.
    /// Typically maps to degrees based on camera model.
    const PAN_RANGE: Range<i16>;

    /// Valid range for tilt position in VISCA units.
    /// Typically maps to degrees based on camera model.
    const TILT_RANGE: Range<i16>;

    /// Maximum pan speed (0x01-0x18 for most cameras).
    const MAX_PAN_SPEED: u8;

    /// Maximum tilt speed (0x01-0x14 for most cameras).
    const MAX_TILT_SPEED: u8;

    /// Whether camera can pan and tilt simultaneously.
    /// Some older cameras may have limitations.
    const PAN_TILT_SIMULTANEOUS: bool = true;

    /// Time needed for preset recovery after recall.
    /// Some cameras need a delay after recalling presets.
    const PRESET_RECOVERY_TIME: Duration = Duration::from_millis(0);

    /// Conversion factor from degrees to VISCA units for pan.
    /// This is camera-specific based on the pan range and degrees coverage.
    const PAN_DEGREES_TO_UNITS: f32;

    /// Conversion factor from degrees to VISCA units for tilt.
    /// This is camera-specific based on the tilt range and degrees coverage.
    const TILT_DEGREES_TO_UNITS: f32;

    /// Coordinate system used by the camera.
    /// Most cameras use SignedCentered, but some legacy models use UnsignedCentered.
    const COORDINATE_SYSTEM: CoordinateSystem = CoordinateSystem::SignedCentered;
}

/// Extension trait that adds validation methods to cameras with pan/tilt support.
pub trait PanTiltExt: PanTilt {
    /// Validate a pan position is within range.
    fn validate_pan(&self, pan: i16) -> Result<i16, ValidationError> {
        if Self::PAN_RANGE.contains(&pan) {
            Ok(pan)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "pan",
                value: pan as f64,
                min: Self::PAN_RANGE.start as f64,
                max: (Self::PAN_RANGE.end - 1) as f64,
            })
        }
    }

    /// Validate a tilt position is within range.
    fn validate_tilt(&self, tilt: i16) -> Result<i16, ValidationError> {
        if Self::TILT_RANGE.contains(&tilt) {
            Ok(tilt)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "tilt",
                value: tilt as f64,
                min: Self::TILT_RANGE.start as f64,
                max: (Self::TILT_RANGE.end - 1) as f64,
            })
        }
    }

    /// Clamp pan speed to valid range.
    fn validate_pan_speed(&self, speed: u8) -> u8 {
        speed.min(Self::MAX_PAN_SPEED).max(1)
    }

    /// Clamp tilt speed to valid range.
    fn validate_tilt_speed(&self, speed: u8) -> u8 {
        speed.min(Self::MAX_TILT_SPEED).max(1)
    }

    /// Convert degrees to VISCA units for pan.
    fn degrees_to_pan_units(&self, degrees: f32) -> i16 {
        (degrees * Self::PAN_DEGREES_TO_UNITS) as i16
    }

    /// Convert VISCA units to degrees for pan.
    fn pan_units_to_degrees(&self, units: i16) -> f32 {
        units as f32 / Self::PAN_DEGREES_TO_UNITS
    }

    /// Convert degrees to VISCA units for tilt.
    fn degrees_to_tilt_units(&self, degrees: f32) -> i16 {
        (degrees * Self::TILT_DEGREES_TO_UNITS) as i16
    }

    /// Convert VISCA units to degrees for tilt.
    fn tilt_units_to_degrees(&self, units: i16) -> f32 {
        units as f32 / Self::TILT_DEGREES_TO_UNITS
    }
}

// Automatic implementation for all types that support pan/tilt
impl<T: PanTilt> PanTiltExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCamera;

    impl PanTilt for TestCamera {
        const PAN_RANGE: Range<i16> = -170..171;
        const TILT_RANGE: Range<i16> = -30..91;
        const MAX_PAN_SPEED: u8 = 24;
        const MAX_TILT_SPEED: u8 = 24;
        const PAN_DEGREES_TO_UNITS: f32 = 100.0;
        const TILT_DEGREES_TO_UNITS: f32 = 100.0;
    }

    #[test]
    fn test_pan_validation() {
        let camera = TestCamera;

        assert!(camera.validate_pan(0).is_ok());
        assert!(camera.validate_pan(170).is_ok());
        assert!(camera.validate_pan(-170).is_ok());
        assert!(camera.validate_pan(171).is_err());
        assert!(camera.validate_pan(-171).is_err());
    }

    #[test]
    fn test_speed_validation() {
        let camera = TestCamera;

        assert_eq!(camera.validate_pan_speed(10), 10);
        assert_eq!(camera.validate_pan_speed(30), 24);
        assert_eq!(camera.validate_pan_speed(0), 1);
    }

    #[test]
    fn test_degree_conversion() {
        let camera = TestCamera;

        assert_eq!(camera.degrees_to_pan_units(45.0), 4500);
        assert_eq!(camera.pan_units_to_degrees(4500), 45.0);
    }
}
