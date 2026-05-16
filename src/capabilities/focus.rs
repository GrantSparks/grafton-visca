//! Focus capability trait and associated types.

use crate::{capabilities::ValidationError, UnitInterval};

/// Trait for cameras that support focus control.
///
/// This trait defines the constants and capabilities for focus operations.
/// Cameras implementing this trait gain access to focus control methods.
pub trait Focus {
    /// Minimum focus position (near limit) in VISCA units.
    const FOCUS_NEAR_LIMIT: u16;

    /// Maximum focus position (far limit) in VISCA units.
    const FOCUS_FAR_LIMIT: u16;

    /// Whether camera supports auto focus mode.
    const SUPPORTS_AUTO_FOCUS: bool;

    /// Whether camera supports one-push auto focus.
    /// This triggers a single auto focus operation then returns to manual.
    const SUPPORTS_ONE_PUSH_FOCUS: bool;

    /// Whether camera supports focus zone selection.
    /// Allows selecting which part of image to focus on.
    const SUPPORTS_FOCUS_ZONE: bool = false;

    /// Maximum focus speed for manual focus operations.
    /// Usually 0-7 where 0 is slowest, 7 is fastest.
    const MAX_FOCUS_SPEED: u8 = 7;

    /// Whether camera supports auto focus sensitivity adjustment.
    const SUPPORTS_AF_SENSITIVITY: bool = false;

    /// Whether camera supports the focus near limit inquiry command.
    /// Some cameras (e.g., PTZOptics G2) can accept the near limit set
    /// command but don't support reading it back via VISCA inquiry.
    const SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY: bool = true;
}

/// Extension trait that adds validation methods to cameras with focus support.
pub trait FocusExt: Focus {
    /// Validate a focus position is within range.
    fn validate_focus_position(&self, position: u16) -> Result<u16, ValidationError> {
        if position >= Self::FOCUS_NEAR_LIMIT && position <= Self::FOCUS_FAR_LIMIT {
            Ok(position)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "focus position",
                value: position as f64,
                min: Self::FOCUS_NEAR_LIMIT as f64,
                max: Self::FOCUS_FAR_LIMIT as f64,
            })
        }
    }

    /// Validate and clamp focus speed.
    fn validate_focus_speed(&self, speed: u8) -> u8 {
        speed.min(Self::MAX_FOCUS_SPEED)
    }

    /// Convert a normalized focus position to VISCA units.
    fn normalized_to_focus_units(&self, normalized: UnitInterval) -> Result<u16, ValidationError> {
        let range = Self::FOCUS_FAR_LIMIT - Self::FOCUS_NEAR_LIMIT;
        let position = Self::FOCUS_NEAR_LIMIT + (normalized.value() * range as f32) as u16;
        Ok(position)
    }

    /// Convert VISCA units to a normalized focus position.
    fn focus_units_to_normalized(&self, units: u16) -> Result<UnitInterval, ValidationError> {
        self.validate_focus_position(units)?;
        let range = Self::FOCUS_FAR_LIMIT - Self::FOCUS_NEAR_LIMIT;
        UnitInterval::new((units - Self::FOCUS_NEAR_LIMIT) as f32 / range as f32).map_err(|_| {
            ValidationError::InvalidValue {
                parameter: "focus position",
                message: "could not convert VISCA units to a unit interval".into(),
            }
        })
    }

    /// Check if auto focus is available.
    fn can_auto_focus(&self) -> bool {
        Self::SUPPORTS_AUTO_FOCUS
    }

    /// Check if one-push focus is available.
    fn can_one_push_focus(&self) -> bool {
        Self::SUPPORTS_ONE_PUSH_FOCUS
    }
}

// Automatic implementation for all types that support focus
impl<T: Focus> FocusExt for T {}

// Note: FocusZone and AutoFocusSensitivity enums are defined in the command module
// and re-exported from the crate root. This avoids duplication.

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    struct TestCamera;

    impl Focus for TestCamera {
        const FOCUS_NEAR_LIMIT: u16 = 0x1000;
        const FOCUS_FAR_LIMIT: u16 = 0xF000;
        const SUPPORTS_AUTO_FOCUS: bool = true;
        const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
    }

    #[test]
    fn test_focus_validation() {
        let camera = TestCamera;

        assert!(camera.validate_focus_position(0x1000).is_ok());
        assert!(camera.validate_focus_position(0xF000).is_ok());
        assert!(camera.validate_focus_position(0x8000).is_ok());
        assert!(camera.validate_focus_position(0x0FFF).is_err());
        assert!(camera.validate_focus_position(0xF001).is_err());
    }

    #[test]
    fn test_normalized_conversion() {
        let camera = TestCamera;

        assert_eq!(
            camera
                .normalized_to_focus_units(UnitInterval::ZERO)
                .expect("0.0 is valid normalized value"),
            0x1000
        );
        assert_eq!(
            camera
                .normalized_to_focus_units(UnitInterval::ONE)
                .expect("1.0 is valid normalized value"),
            0xF000
        );

        // Test round trip
        let pos = camera
            .normalized_to_focus_units(UnitInterval::new(0.5).expect("0.5 is valid"))
            .expect("0.5 is valid normalized value");
        let normalized = camera
            .focus_units_to_normalized(pos)
            .expect("valid focus units convert to a unit interval");
        assert!((normalized.value() - 0.5).abs() < 0.01);
        assert!(camera.focus_units_to_normalized(0x0FFF).is_err());
    }
}
