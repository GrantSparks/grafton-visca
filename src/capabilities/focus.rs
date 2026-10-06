//! Focus capability trait and associated types.

use crate::{
    capabilities::{CapabilityRange, ValidationError},
    command::FocusZone,
    UnitInterval,
};

/// The `CAM_AFZone` values every focus-zone source documents: Top, Center and
/// Bottom (`8x 01 04 AA 00/01/02 FF`).
pub(crate) const DOCUMENTED_FOCUS_ZONES: &[FocusZone] =
    &[FocusZone::Top, FocusZone::Center, FocusZone::Bottom];

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

    /// Whether camera supports the focus-zone inquiry command.
    ///
    /// This is distinct from focus-zone selection ([`Self::FOCUS_ZONES`]):
    /// some model references document selection but not a reliable status
    /// response.
    const SUPPORTS_FOCUS_ZONE_INQUIRY: bool = false;

    /// Focus-zone values the typed setter may send to this camera; empty when
    /// the camera has no focus-zone selection (the default).
    ///
    /// A camera with the documented `CAM_AFZone` rows lists Top, Center and
    /// Bottom. Add a value such as [`FocusZone::Zone03`]
    /// only for a camera whose evidence shows it accepts that value. The
    /// focus-zone inquiry decodes every known value whatever this list says,
    /// because decoding a reply sends nothing.
    const FOCUS_ZONES: &'static [FocusZone] = &[];

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
    ///
    /// A profile whose `FOCUS_NEAR_LIMIT` exceeds its `FOCUS_FAR_LIMIT` fails to
    /// compile a call to this method.
    fn validate_focus_position(&self, position: u16) -> Result<u16, ValidationError> {
        const { CapabilityRange::<u16>::new(Self::FOCUS_NEAR_LIMIT, Self::FOCUS_FAR_LIMIT) }
            .validate("focus position", position)
    }

    /// Validate a focus speed against `0..=MAX_FOCUS_SPEED`.
    ///
    /// Rejects out-of-range speeds instead of clamping them, matching the
    /// request path (`Capabilities::focus_speed`).
    fn validate_focus_speed(&self, speed: u8) -> Result<u8, ValidationError> {
        CapabilityRange::<u8>::new(0, Self::MAX_FOCUS_SPEED).validate("focus speed", speed)
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
            ValidationError::invalid_value(
                "focus position",
                "could not convert VISCA units to a unit interval",
            )
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
    fn focus_speed_validation_rejects_instead_of_clamping() {
        let camera = TestCamera;

        assert_eq!(camera.validate_focus_speed(0), Ok(0));
        assert_eq!(camera.validate_focus_speed(7), Ok(7));
        assert_eq!(
            camera.validate_focus_speed(8),
            Err(ValidationError::out_of_range("focus speed", 8.0, 0.0, 7.0))
        );
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
