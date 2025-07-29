//! Exposure capability trait and associated types.

use std::ops::Range;

use crate::capabilities::ValidationError;

/// Trait for cameras that support exposure control.
///
/// This trait defines the constants and capabilities for exposure settings
/// including iris, shutter speed, gain, and exposure compensation.
#[allow(dead_code)]
pub trait Exposure {
    /// Valid range for iris values in VISCA units.
    const IRIS_RANGE: Range<u16>;

    /// Supported shutter speeds as VISCA values.
    /// Each camera model has specific supported speeds.
    const SHUTTER_SPEEDS: &'static [ShutterSpeed];

    /// Valid range for gain values.
    const GAIN_RANGE: Range<u8>;

    /// Whether camera supports auto exposure mode.
    const SUPPORTS_AUTO_EXPOSURE: bool;

    /// Whether camera supports backlight compensation.
    const SUPPORTS_BACKLIGHT_COMP: bool;

    /// Whether camera supports exposure compensation.
    const SUPPORTS_EXPOSURE_COMP: bool = false;

    /// Range for exposure compensation if supported.
    /// Typically -7 to +7 in steps.
    const EXPOSURE_COMP_RANGE: Range<i8> = -7..8;

    /// Whether camera supports wide dynamic range.
    const SUPPORTS_WDR: bool = false;
}

/// Extension trait that adds validation methods to cameras with exposure support.
#[allow(dead_code)]
pub trait ExposureExt: Exposure {
    /// Validate iris value is within range.
    fn validate_iris(&self, iris: u16) -> Result<u16, ValidationError> {
        if Self::IRIS_RANGE.contains(&iris) {
            Ok(iris)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "iris",
                value: iris as f64,
                min: Self::IRIS_RANGE.start as f64,
                max: (Self::IRIS_RANGE.end - 1) as f64,
            })
        }
    }

    /// Validate gain value is within range.
    fn validate_gain(&self, gain: u8) -> Result<u8, ValidationError> {
        if Self::GAIN_RANGE.contains(&gain) {
            Ok(gain)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "gain",
                value: gain as f64,
                min: Self::GAIN_RANGE.start as f64,
                max: (Self::GAIN_RANGE.end - 1) as f64,
            })
        }
    }

    /// Find the closest supported shutter speed.
    fn find_shutter_speed(&self, target_value: u16) -> Option<&ShutterSpeed> {
        Self::SHUTTER_SPEEDS
            .iter()
            .min_by_key(|speed| (speed.value as i32 - target_value as i32).abs())
    }

    /// Validate shutter speed is supported.
    fn validate_shutter_speed(&self, value: u16) -> Result<u16, ValidationError> {
        if Self::SHUTTER_SPEEDS.iter().any(|s| s.value == value) {
            Ok(value)
        } else {
            Err(ValidationError::InvalidValue {
                parameter: "shutter speed",
                message: format!("Unsupported shutter speed value: {}", value),
            })
        }
    }

    /// Validate exposure compensation value.
    fn validate_exposure_comp(&self, value: i8) -> Result<i8, ValidationError> {
        if !Self::SUPPORTS_EXPOSURE_COMP {
            return Err(ValidationError::NotSupported("exposure compensation"));
        }

        if Self::EXPOSURE_COMP_RANGE.contains(&value) {
            Ok(value)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "exposure compensation",
                value: value as f64,
                min: Self::EXPOSURE_COMP_RANGE.start as f64,
                max: (Self::EXPOSURE_COMP_RANGE.end - 1) as f64,
            })
        }
    }

    /// Convert F-stop to iris VISCA units.
    fn fstop_to_iris_units(&self, fstop: f32) -> Result<u16, ValidationError> {
        // This is camera-specific and would need proper calibration
        // This is a simplified example
        let iris = match fstop {
            f if f <= 1.8 => Self::IRIS_RANGE.end - 1,
            f if f >= 11.0 => Self::IRIS_RANGE.start,
            f => {
                let range = Self::IRIS_RANGE.end - Self::IRIS_RANGE.start;
                let normalized = 1.0 - ((f - 1.8) / (11.0 - 1.8));
                Self::IRIS_RANGE.start + (normalized * range as f32) as u16
            }
        };

        self.validate_iris(iris)
    }
}

// Automatic implementation for all types that support exposure
impl<T: Exposure> ExposureExt for T {}

/// Shutter speed definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutterSpeed {
    /// Display label (e.g., "1/30", "1/60").
    pub label: &'static str,
    /// VISCA protocol value.
    pub value: u16,
}

impl ShutterSpeed {
    /// Create a new shutter speed.
    pub const fn new(label: &'static str, value: u16) -> Self {
        Self { label, value }
    }
}

// Note: ExposureMode and DynamicRangeLevel enums are defined in the command module
// and re-exported from the crate root. This avoids duplication.

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const TEST_SHUTTER_SPEEDS: &[ShutterSpeed] = &[
        ShutterSpeed::new("1/30", 0x00),
        ShutterSpeed::new("1/60", 0x01),
        ShutterSpeed::new("1/100", 0x02),
        ShutterSpeed::new("1/250", 0x03),
        ShutterSpeed::new("1/500", 0x04),
        ShutterSpeed::new("1/1000", 0x05),
    ];

    struct TestCamera;

    impl Exposure for TestCamera {
        const IRIS_RANGE: Range<u16> = 0x00..0x1D;
        const SHUTTER_SPEEDS: &'static [ShutterSpeed] = TEST_SHUTTER_SPEEDS;
        const GAIN_RANGE: Range<u8> = 0..16;
        const SUPPORTS_AUTO_EXPOSURE: bool = true;
        const SUPPORTS_BACKLIGHT_COMP: bool = true;
        const SUPPORTS_EXPOSURE_COMP: bool = true;
    }

    #[test]
    fn test_iris_validation() {
        let camera = TestCamera;

        assert!(camera.validate_iris(0x00).is_ok());
        assert!(camera.validate_iris(0x1C).is_ok());
        assert!(camera.validate_iris(0x1D).is_err());
    }

    #[test]
    fn test_shutter_speed_lookup() {
        let camera = TestCamera;

        assert_eq!(
            camera
                .find_shutter_speed(0x01)
                .expect("0x01 is a valid shutter speed")
                .label,
            "1/60"
        );
        assert_eq!(
            camera
                .find_shutter_speed(0x02)
                .expect("0x02 is a valid shutter speed")
                .label,
            "1/100"
        );

        // Should find closest
        assert_eq!(
            camera
                .find_shutter_speed(0x10)
                .expect("should find closest shutter speed")
                .label,
            "1/1000"
        );
    }
}
