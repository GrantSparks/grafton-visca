//! Inquiry conversion utilities for converting raw VISCA values to user-friendly formats.
//!
//! This module provides helpers for converting raw inquiry responses (raw VISCA values)
//! to more intuitive representations like degrees and normalized values, as part of the
//! runtime-agnostic modernization of `grafton-visca`.

use crate::{
    camera::PanTiltPosition,
    types::{PanPosition, TiltPosition, ZoomPosition},
    units::{Degrees, UnitInterval},
};

/// Represents a pan/tilt position in raw VISCA units.
///
/// This is the format returned directly from camera inquiries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PanTiltPositionRaw {
    /// Pan position in raw VISCA units (-2448 to +2448).
    pub pan: i16,
    /// Tilt position in raw VISCA units (-432 to +1296).
    pub tilt: i16,
}

impl PanTiltPositionRaw {
    /// Creates a new raw pan/tilt position.
    pub const fn new(pan: i16, tilt: i16) -> Self {
        Self { pan, tilt }
    }

    /// Converts raw pan/tilt position to degrees.
    ///
    /// Uses the standard VISCA conversion formulas:
    /// - Pan: -170° to +170° mapped from -2448 to +2448
    /// - Tilt: -30° to +90° mapped from -432 to +1296
    ///
    /// # Example
    /// ```ignore
    /// let raw_pos = PanTiltPositionRaw::new(1224, 648);
    /// let deg_pos = raw_pos.as_degrees();
    /// // deg_pos.pan ≈ 85°, deg_pos.tilt ≈ 45°
    /// ```
    pub fn as_degrees(&self) -> PanTiltPositionDeg {
        // Use the existing conversion methods from PanPosition and TiltPosition
        let pan_pos = PanPosition::new(self.pan).unwrap_or(PanPosition::CENTER);
        let tilt_pos = TiltPosition::new(self.tilt).unwrap_or(TiltPosition::CENTER);

        PanTiltPositionDeg {
            pan: Degrees(pan_pos.to_degrees()),
            tilt: Degrees(tilt_pos.to_degrees()),
        }
    }

    /// Converts raw pan/tilt position to degrees with profile-specific adjustments.
    ///
    /// Some camera profiles may have different conversion factors or ranges.
    /// This method allows for profile-aware conversion.
    ///
    /// # Parameters
    /// - `profile`: The camera profile to use for conversion
    ///
    /// # Note
    /// Currently uses standard VISCA conversion. Profile-specific adjustments
    /// will be added as needed for different camera models.
    pub fn as_degrees_with_profile<P: crate::capabilities::Profile>(
        &self,
        _profile: &P,
    ) -> PanTiltPositionDeg {
        // For now, use standard conversion
        // In future, profiles can override conversion factors
        self.as_degrees()
    }
}

impl From<PanTiltPosition> for PanTiltPositionRaw {
    fn from(pos: PanTiltPosition) -> Self {
        Self {
            pan: pos.pan,
            tilt: pos.tilt,
        }
    }
}

/// Represents a pan/tilt position in degrees.
///
/// This is the user-friendly representation after converting from raw VISCA units.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PanTiltPositionDeg {
    /// Pan position in degrees (-170° to +170°).
    pub pan: Degrees<f32>,
    /// Tilt position in degrees (-30° to +90°).
    pub tilt: Degrees<f32>,
}

impl PanTiltPositionDeg {
    /// Creates a new pan/tilt position in degrees.
    pub const fn new(pan: Degrees<f32>, tilt: Degrees<f32>) -> Self {
        Self { pan, tilt }
    }

    /// Converts degrees position to raw VISCA units.
    ///
    /// # Errors
    /// Returns an error if the degree values are outside valid ranges.
    pub fn to_raw(&self) -> Result<PanTiltPositionRaw, crate::Error> {
        let pan_pos = PanPosition::from_degrees(self.pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(self.tilt.0)?;

        Ok(PanTiltPositionRaw {
            pan: pan_pos.value(),
            tilt: tilt_pos.value(),
        })
    }
}

/// Zoom domain for normalization.
///
/// Determines how zoom values are normalized to 0.0-1.0 range.
/// The actual max values are profile-specific (e.g., 0x4000 optical for
/// PTZOptics raw VISCA profiles, higher endpoints only for profiles with
/// validated digital zoom ranges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum ZoomDomain {
    /// Optical zoom only.
    ///
    /// Normalizes across the optical zoom range only.
    /// Maximum zoom is limited to the optical telephoto end.
    Optical,
    /// Combined optical and digital zoom.
    ///
    /// Normalizes across the full zoom range including digital zoom.
    /// Not all cameras support digital zoom.
    OpticalPlusDigital,
}

/// Extension trait for ZoomPosition to add profile-aware normalization.
pub trait ZoomPositionExt {
    /// Normalizes the zoom position to 0.0-1.0 range based on the specified domain.
    ///
    /// The max values come from the camera profile's zoom capability constants,
    /// ensuring correct normalization for different camera models.
    ///
    /// # Parameters
    /// - `domain`: Determines whether to normalize against optical or full zoom range
    /// - `optical_max`: The camera's maximum optical zoom value (from profile)
    /// - `digital_max`: The camera's maximum digital zoom value, if supported
    ///
    /// # Returns
    /// A normalized value where:
    /// - 0.0 = wide end (0x0000)
    /// - 1.0 = telephoto end for the selected domain
    fn normalize_with_max(
        &self,
        domain: ZoomDomain,
        optical_max: u16,
        digital_max: Option<u16>,
    ) -> UnitInterval;

    /// Creates a zoom position from a normalized value (0.0-1.0).
    ///
    /// # Parameters
    /// - `normalized`: Value between 0.0 and 1.0
    /// - `domain`: Determines the zoom range to map to
    /// - `optical_max`: The camera's maximum optical zoom value (from profile)
    /// - `digital_max`: The camera's maximum digital zoom value, if supported
    ///
    /// # Errors
    /// Returns an error if the normalized value is outside 0.0-1.0 range.
    fn from_normalized(
        normalized: UnitInterval,
        domain: ZoomDomain,
        optical_max: u16,
        digital_max: Option<u16>,
    ) -> Result<ZoomPosition, crate::Error>;
}

impl ZoomPositionExt for ZoomPosition {
    fn normalize_with_max(
        &self,
        domain: ZoomDomain,
        optical_max: u16,
        digital_max: Option<u16>,
    ) -> UnitInterval {
        let max = match domain {
            ZoomDomain::Optical => optical_max,
            ZoomDomain::OpticalPlusDigital => digital_max.unwrap_or(optical_max),
        };

        if max == 0 {
            return UnitInterval::ZERO;
        }

        let normalized = (self.value() as f32) / (max as f32);
        UnitInterval::new(normalized.clamp(0.0, 1.0)).unwrap_or(UnitInterval::ZERO)
    }

    fn from_normalized(
        normalized: UnitInterval,
        domain: ZoomDomain,
        optical_max: u16,
        digital_max: Option<u16>,
    ) -> Result<ZoomPosition, crate::Error> {
        zoom_from_normalized(normalized, domain, optical_max, digital_max)
    }
}

impl ZoomPositionExt for () {
    fn normalize_with_max(
        &self,
        _domain: ZoomDomain,
        _optical_max: u16,
        _digital_max: Option<u16>,
    ) -> UnitInterval {
        UnitInterval::ZERO
    }

    fn from_normalized(
        normalized: UnitInterval,
        domain: ZoomDomain,
        optical_max: u16,
        digital_max: Option<u16>,
    ) -> Result<ZoomPosition, crate::Error> {
        zoom_from_normalized(normalized, domain, optical_max, digital_max)
    }
}

/// Helper function to create a ZoomPosition from normalized value.
pub fn zoom_from_normalized(
    normalized: UnitInterval,
    domain: ZoomDomain,
    optical_max: u16,
    digital_max: Option<u16>,
) -> Result<ZoomPosition, crate::Error> {
    let max = match domain {
        ZoomDomain::Optical => optical_max,
        ZoomDomain::OpticalPlusDigital => digital_max.unwrap_or(optical_max),
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let raw_value = (normalized.value() * max as f32).round() as u16;
    ZoomPosition::new(raw_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pan_tilt_raw_to_degrees() {
        // Test center position
        let raw = PanTiltPositionRaw::new(0, 0);
        let deg = raw.as_degrees();
        assert_eq!(deg.pan.0, 0.0);
        assert_eq!(deg.tilt.0, 0.0);

        // Test maximum pan right (+170°)
        let raw = PanTiltPositionRaw::new(2448, 0);
        let deg = raw.as_degrees();
        assert!((deg.pan.0 - 170.0).abs() < 0.1);

        // Test maximum pan left (-170°)
        let raw = PanTiltPositionRaw::new(-2448, 0);
        let deg = raw.as_degrees();
        assert!((deg.pan.0 + 170.0).abs() < 0.1);

        // Test maximum tilt down (+90°)
        let raw = PanTiltPositionRaw::new(0, 1296);
        let deg = raw.as_degrees();
        assert!((deg.tilt.0 - 90.0).abs() < 0.1);

        // Test maximum tilt up (-30°)
        let raw = PanTiltPositionRaw::new(0, -432);
        let deg = raw.as_degrees();
        assert!((deg.tilt.0 + 30.0).abs() < 0.1);
    }

    #[test]
    fn test_pan_tilt_deg_to_raw() -> Result<(), crate::Error> {
        // Test roundtrip conversion
        let deg = PanTiltPositionDeg::new(Degrees(45.0), Degrees(30.0));
        let raw = deg.to_raw()?;
        let deg2 = raw.as_degrees();

        assert!((deg.pan.0 - deg2.pan.0).abs() < 1.0);
        assert!((deg.tilt.0 - deg2.tilt.0).abs() < 1.0);
        Ok(())
    }

    #[test]
    fn test_zoom_normalization() -> Result<(), crate::Error> {
        // Use PtzOpticsG2-style values for testing
        let optical_max: u16 = 0x4000;
        let digital_max: Option<u16> = Some(0x7000);

        // Test wide end
        let zoom = ZoomPosition::new(0x0000)?;
        assert_eq!(
            zoom.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)
                .value(),
            0.0
        );
        assert_eq!(
            zoom.normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max)
                .value(),
            0.0
        );

        // Test optical telephoto end
        let zoom = ZoomPosition::new(optical_max)?;
        assert_eq!(
            zoom.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)
                .value(),
            1.0
        );
        assert!(
            (zoom
                .normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max)
                .value()
                - 0.571)
                .abs()
                < 0.01
        );

        // Test digital telephoto end
        let zoom = ZoomPosition::new(0x7000)?;
        assert!(
            zoom.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)
                .value()
                >= 1.0
        ); // Clamped to 1.0
        assert_eq!(
            zoom.normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max)
                .value(),
            1.0
        );

        // Test middle position
        let zoom = ZoomPosition::new(0x2000)?;
        assert_eq!(
            zoom.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)
                .value(),
            0.5
        );
        assert!(
            (zoom
                .normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max)
                .value()
                - 0.286)
                .abs()
                < 0.01
        );
        Ok(())
    }

    #[test]
    fn test_zoom_from_normalized() -> Result<(), crate::Error> {
        let optical_max: u16 = 0x4000;
        let digital_max: Option<u16> = Some(0x7000);

        // Test optical domain
        let norm = UnitInterval::new(0.5)?;
        let zoom = zoom_from_normalized(norm, ZoomDomain::Optical, optical_max, digital_max)?;
        assert_eq!(zoom.value(), 0x2000);

        // Test digital domain
        let norm = UnitInterval::new(1.0)?;
        let zoom = zoom_from_normalized(
            norm,
            ZoomDomain::OpticalPlusDigital,
            optical_max,
            digital_max,
        )?;
        assert_eq!(zoom.value(), 0x7000);

        // Test edge cases
        let norm = UnitInterval::new(0.0)?;
        let zoom = zoom_from_normalized(norm, ZoomDomain::Optical, optical_max, digital_max)?;
        assert_eq!(zoom.value(), 0x0000);
        Ok(())
    }

    #[test]
    fn test_unit_interval_validation() -> Result<(), crate::Error> {
        assert!(UnitInterval::new(0.0).is_ok());
        assert!(UnitInterval::new(0.5).is_ok());
        assert!(UnitInterval::new(1.0).is_ok());

        assert!(UnitInterval::new(-0.1).is_err());
        assert!(UnitInterval::new(1.1).is_err());
        assert!(UnitInterval::new(f32::NAN).is_err());
        assert!(UnitInterval::new(f32::INFINITY).is_err());
        assert!(UnitInterval::new(f32::NEG_INFINITY).is_err());

        let norm = UnitInterval::new(0.75)?;
        assert_eq!(norm.to_percentage(), 75);
        Ok(())
    }
}
