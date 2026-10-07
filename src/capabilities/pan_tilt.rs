//! Pan/Tilt capability trait and associated types.

use std::time::Duration;

use crate::capabilities::{CapabilityRange, CoordinateSystem, ValidationError};

/// Profile-owned wire framing for pan/tilt position commands and inquiries.
///
/// The baseline VISCA form carries two speed bytes and four nibbles per axis.
/// Some camera families use a different position payload even though they keep
/// the usual pan/tilt opcodes.  This discriminator keeps that variation a
/// profile fact rather than scattering model checks through encoders and
/// decoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum PanTiltWireCodec {
    /// Standard VISCA: two speed bytes and four nibbles for each axis.
    #[default]
    StandardVisca,
    /// Sony BRC-300: one speed byte, fixed `00`, five signed pan nibbles,
    /// and four signed tilt nibbles.
    SonyBrc300,
}

/// Trait for cameras that support pan and tilt movement.
///
/// This trait defines the constants and capabilities for pan/tilt operations.
/// Cameras implementing this trait gain access to pan/tilt movement methods.
pub trait PanTilt {
    /// Valid range for pan position in VISCA units.
    /// Typically maps to degrees based on camera model.
    const PAN_RANGE: CapabilityRange<i32>;

    /// Valid range for tilt position in VISCA units.
    /// Typically maps to degrees based on camera model.
    const TILT_RANGE: CapabilityRange<i32>;

    /// Maximum pan speed (0x01-0x18 for most cameras).
    const MAX_PAN_SPEED: u8;

    /// Maximum tilt speed (0x01-0x14 for most cameras; up to 0x18 where documented).
    const MAX_TILT_SPEED: u8;

    /// Whether camera can pan and tilt simultaneously.
    /// Some older cameras may have limitations.
    const PAN_TILT_SIMULTANEOUS: bool = true;

    /// Time needed for preset recovery after recall.
    /// Some cameras need a delay after recalling presets.
    const PRESET_RECOVERY_TIME: Duration = Duration::from_millis(0);

    /// Signed scale from the library's logical pan degrees to raw camera units.
    ///
    /// A positive scale maps positive (rightward) degrees to increasing raw
    /// units. A negative scale maps them to decreasing raw units for cameras
    /// whose documented wire-axis polarity is reversed. The scale must be
    /// finite and nonzero.
    const PAN_DEGREES_TO_UNITS: f32;

    /// Signed scale from the library's logical tilt degrees to raw camera units.
    ///
    /// A positive scale maps positive (upward) degrees to increasing raw
    /// units. A negative scale maps them to decreasing raw units for cameras
    /// whose documented wire-axis polarity is reversed. The scale must be
    /// finite and nonzero.
    const TILT_DEGREES_TO_UNITS: f32;

    /// Coordinate system used by the camera.
    /// Most cameras use SignedCentered, but some legacy models use UnsignedCentered.
    const COORDINATE_SYSTEM: CoordinateSystem = CoordinateSystem::SignedCentered;

    /// Position-command and inquiry framing used by the profile.
    const PAN_TILT_WIRE_CODEC: PanTiltWireCodec = PanTiltWireCodec::StandardVisca;
}

/// Extension trait that adds validation methods to cameras with pan/tilt support.
pub trait PanTiltExt: PanTilt {
    /// Validate a pan position is within range.
    fn validate_pan(&self, pan: i32) -> Result<i32, ValidationError> {
        Self::PAN_RANGE.validate("pan", pan)
    }

    /// Validate a tilt position is within range.
    fn validate_tilt(&self, tilt: i32) -> Result<i32, ValidationError> {
        Self::TILT_RANGE.validate("tilt", tilt)
    }

    /// Validate a pan speed against `1..=MAX_PAN_SPEED`.
    ///
    /// Rejects out-of-range speeds instead of clamping them, matching the
    /// request path (`Capabilities::pan_speed`).
    /// A profile whose `MAX_PAN_SPEED` is `0` has no valid speed and fails to
    /// compile a call to this method.
    fn validate_pan_speed(&self, speed: u8) -> Result<u8, ValidationError> {
        const { CapabilityRange::<u8>::new(1, Self::MAX_PAN_SPEED) }.validate("pan speed", speed)
    }

    /// Validate a tilt speed against `1..=MAX_TILT_SPEED`.
    ///
    /// Rejects out-of-range speeds instead of clamping them, matching the
    /// request path (`Capabilities::tilt_speed`).
    /// A profile whose `MAX_TILT_SPEED` is `0` has no valid speed and fails to
    /// compile a call to this method.
    fn validate_tilt_speed(&self, speed: u8) -> Result<u8, ValidationError> {
        const { CapabilityRange::<u8>::new(1, Self::MAX_TILT_SPEED) }.validate("tilt speed", speed)
    }

    /// Converts a pan angle to raw units with this profile's scale.
    ///
    /// Delegates to [`PanTiltCoordinateConversion::pan_units`], the crate's
    /// single conversion and rounding rule. Returns `None` for a non-finite
    /// angle or a result outside `i32`; range checking is [`Self::validate_pan`].
    ///
    /// [`PanTiltCoordinateConversion::pan_units`]: crate::PanTiltCoordinateConversion::pan_units
    fn degrees_to_pan_units(&self, degrees: f32) -> Option<i32> {
        crate::PanTiltCoordinateConversion::for_profile::<Self>().pan_units(degrees)
    }

    /// Converts raw pan units to degrees with this profile's scale.
    fn pan_units_to_degrees(&self, units: i32) -> f32 {
        crate::PanTiltCoordinateConversion::for_profile::<Self>().pan_degrees(units)
    }

    /// Converts a tilt angle to raw units with this profile's scale.
    ///
    /// Delegates to [`PanTiltCoordinateConversion::tilt_units`]; see
    /// [`Self::degrees_to_pan_units`].
    ///
    /// [`PanTiltCoordinateConversion::tilt_units`]: crate::PanTiltCoordinateConversion::tilt_units
    fn degrees_to_tilt_units(&self, degrees: f32) -> Option<i32> {
        crate::PanTiltCoordinateConversion::for_profile::<Self>().tilt_units(degrees)
    }

    /// Converts raw tilt units to degrees with this profile's scale.
    fn tilt_units_to_degrees(&self, units: i32) -> f32 {
        crate::PanTiltCoordinateConversion::for_profile::<Self>().tilt_degrees(units)
    }
}

// Automatic implementation for all types that support pan/tilt
impl<T: PanTilt> PanTiltExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCamera;

    impl PanTilt for TestCamera {
        const PAN_RANGE: CapabilityRange<i32> = CapabilityRange::<i32>::new(-170, 170);
        const TILT_RANGE: CapabilityRange<i32> = CapabilityRange::<i32>::new(-30, 90);
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

        assert_eq!(camera.validate_pan_speed(10), Ok(10));
        assert_eq!(camera.validate_pan_speed(1), Ok(1));
        assert_eq!(camera.validate_pan_speed(24), Ok(24));
        assert_eq!(
            camera.validate_pan_speed(30),
            Err(ValidationError::out_of_range("pan speed", 30.0, 1.0, 24.0))
        );
        assert_eq!(
            camera.validate_pan_speed(0),
            Err(ValidationError::out_of_range("pan speed", 0.0, 1.0, 24.0))
        );
        assert_eq!(camera.validate_tilt_speed(24), Ok(24));
        assert_eq!(
            camera.validate_tilt_speed(25),
            Err(ValidationError::out_of_range("tilt speed", 25.0, 1.0, 24.0))
        );
        assert!(camera.validate_tilt_speed(0).is_err());
    }

    #[test]
    fn test_degree_conversion() {
        let camera = TestCamera;

        assert_eq!(camera.degrees_to_pan_units(45.0), Some(4500));
        assert_eq!(camera.degrees_to_pan_units(f32::NAN), None);
        assert_eq!(camera.degrees_to_pan_units(f32::INFINITY), None);
        assert_eq!(camera.degrees_to_pan_units(3.0e7), None);
        assert_eq!(camera.pan_units_to_degrees(4500), 45.0);
    }

    #[test]
    fn signed_profile_scales_reverse_brc300_pan_and_keep_its_tilt() {
        use crate::profiles::SonyBRC300;

        let camera = SonyBRC300;
        assert_eq!(camera.degrees_to_pan_units(45.0), Some(-0x02490));
        assert_eq!(camera.degrees_to_tilt_units(15.0), Some(0x0C30));
        assert_eq!(camera.pan_units_to_degrees(-0x02490), 45.0);
        assert_eq!(camera.tilt_units_to_degrees(0x0C30), 15.0);
    }
}
