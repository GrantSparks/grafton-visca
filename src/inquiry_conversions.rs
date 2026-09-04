//! Inquiry conversion utilities for converting raw VISCA values to user-friendly formats.
//!
//! This module provides helpers for converting raw inquiry responses (raw VISCA values)
//! to more intuitive representations like degrees and normalized values, as part of the
//! runtime-agnostic modernization of `grafton-visca`.

use std::borrow::Cow;

use crate::{
    camera::PanTiltPosition,
    capabilities::Profile,
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
    /// Pan position in raw VISCA units.
    pub pan: i32,
    /// Tilt position in raw VISCA units.
    pub tilt: i32,
}

impl PanTiltPositionRaw {
    /// Creates a new raw pan/tilt position.
    pub const fn new(pan: i32, tilt: i32) -> Self {
        Self { pan, tilt }
    }

    /// Converts raw pan/tilt position to degrees.
    ///
    /// Uses the standard VISCA conversion formulas:
    /// - Pan: -170° to +170° mapped from -2448 to +2448
    /// - Tilt: -30° to +90° mapped from -432 to +1296
    ///
    /// An axis outside those standard `i16` ranges returns `NaN` rather than
    /// silently being treated as the center position. Use
    /// [`Self::as_degrees_with_profile`] for profile-specific coordinates such
    /// as Sony BRC-300's signed 20-bit pan values.
    ///
    /// # Example
    /// ```ignore
    /// let raw_pos = PanTiltPositionRaw::new(1224, 648);
    /// let deg_pos = raw_pos.as_degrees();
    /// // deg_pos.pan ≈ 85°, deg_pos.tilt ≈ 45°
    /// ```
    pub fn as_degrees(&self) -> PanTiltPositionDeg {
        let pan = i16::try_from(self.pan)
            .ok()
            .and_then(|value| PanPosition::new(value).ok())
            .map_or(f32::NAN, PanPosition::to_degrees);
        let tilt = i16::try_from(self.tilt)
            .ok()
            .and_then(|value| TiltPosition::new(value).ok())
            .map_or(f32::NAN, TiltPosition::to_degrees);

        PanTiltPositionDeg {
            pan: Degrees(pan),
            tilt: Degrees(tilt),
        }
    }

    /// Converts raw pan/tilt position to degrees with profile-specific adjustments.
    ///
    /// Some camera profiles may have different signed conversion scales or
    /// ranges. A negative scale preserves a camera's reverse raw-axis
    /// polarity while keeping the library degree convention unchanged.
    ///
    /// # Parameters
    /// - `profile`: The camera profile to use for conversion
    ///
    pub fn as_degrees_with_profile<P: Profile>(&self, _profile: &P) -> PanTiltPositionDeg {
        PanTiltPositionDeg {
            pan: Degrees(self.pan as f32 / P::PAN_DEGREES_TO_UNITS),
            tilt: Degrees(self.tilt as f32 / P::TILT_DEGREES_TO_UNITS),
        }
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
            pan: i32::from(pan_pos.value()),
            tilt: i32::from(tilt_pos.value()),
        })
    }

    /// Converts degrees to raw units using a profile's signed, profile-specified
    /// pan/tilt scales and validated raw coordinate ranges.
    pub fn to_raw_with_profile<P: Profile>(
        &self,
        _profile: &P,
    ) -> Result<PanTiltPositionRaw, crate::Error> {
        if !self.pan.0.is_finite() || !self.tilt.0.is_finite() {
            return Err(crate::Error::InvalidRequest(
                "pan/tilt degree values must be finite".into(),
            ));
        }
        let pan = (self.pan.0 * P::PAN_DEGREES_TO_UNITS).round();
        let tilt = (self.tilt.0 * P::TILT_DEGREES_TO_UNITS).round();
        if !(i32::MIN as f32..=i32::MAX as f32).contains(&pan)
            || !(i32::MIN as f32..=i32::MAX as f32).contains(&tilt)
        {
            return Err(crate::Error::InvalidRequest(
                "converted pan/tilt coordinate exceeds signed 32-bit units".into(),
            ));
        }
        let pan = pan as i32;
        let tilt = tilt as i32;
        if !P::PAN_RANGE.contains(pan) || !P::TILT_RANGE.contains(tilt) {
            return Err(crate::Error::InvalidRequest(
                "converted pan/tilt units are outside the profile range".into(),
            ));
        }
        Ok(PanTiltPositionRaw { pan, tilt })
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
    /// # Errors
    /// Returns an error if the requested domain has no documented maximum, or
    /// if the raw position exceeds the selected domain maximum.
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
    ) -> Result<UnitInterval, crate::Error>;

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
    ) -> Result<UnitInterval, crate::Error> {
        let max = zoom_domain_max(domain, optical_max, digital_max)?;

        if max == 0 {
            return Ok(UnitInterval::ZERO);
        }

        if self.value() > max {
            return Err(crate::Error::InvalidParameter {
                parameter: "zoom position",
                value: Cow::Owned(format!("{:#06X}", self.value())),
                reason: Cow::Owned(format!(
                    "value exceeds {:?} zoom maximum {:#06X}",
                    domain, max
                )),
            });
        }

        let normalized = f64::from(self.value()) / f64::from(max);
        UnitInterval::new(normalized as f32)
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
    ) -> Result<UnitInterval, crate::Error> {
        Ok(UnitInterval::ZERO)
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

/// Returns the documented raw zoom maximum for a normalization domain.
///
/// Optical normalization always uses the profile optical maximum. Digital-domain
/// normalization requires an explicit digital maximum and never falls back to
/// the optical range.
pub fn zoom_domain_max(
    domain: ZoomDomain,
    optical_max: u16,
    digital_max: Option<u16>,
) -> Result<u16, crate::Error> {
    match domain {
        ZoomDomain::Optical => Ok(optical_max),
        ZoomDomain::OpticalPlusDigital => digital_max.ok_or(crate::Error::FeatureNotSupported {
            feature: "optical-plus-digital zoom range",
        }),
    }
}

/// Helper function to create a ZoomPosition from normalized value.
pub fn zoom_from_normalized(
    normalized: UnitInterval,
    domain: ZoomDomain,
    optical_max: u16,
    digital_max: Option<u16>,
) -> Result<ZoomPosition, crate::Error> {
    let max = zoom_domain_max(domain, optical_max, digital_max)?;

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let raw_value = (f64::from(normalized.value()) * f64::from(max)).round() as u16;
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
    fn sony_brc300_profile_conversion_preserves_documented_reverse_axis_polarity(
    ) -> Result<(), crate::Error> {
        use crate::profiles::SonyBRC300;

        let degrees = PanTiltPositionDeg::new(Degrees(45.0), Degrees(-15.0));
        assert_eq!(
            degrees.to_raw_with_profile(&SonyBRC300)?,
            PanTiltPositionRaw::new(-0x02490, 0x0C30)
        );
        let round_trip =
            PanTiltPositionRaw::new(-0x02490, 0x0C30).as_degrees_with_profile(&SonyBRC300);
        assert!((round_trip.pan.0 - 45.0).abs() < f32::EPSILON);
        assert!((round_trip.tilt.0 + 15.0).abs() < f32::EPSILON);

        // The manual's positive raw endpoints are left/up, so the profile
        // maps them to negative library degrees. They also exceed i16 for pan.
        let left_up = PanTiltPositionRaw::new(0x08A58, 0x493D).as_degrees_with_profile(&SonyBRC300);
        assert!(left_up.pan.0 < 0.0);
        assert!(left_up.tilt.0 < 0.0);
        assert!((left_up.pan.0 + 0x08A58 as f32 / 208.0).abs() < f32::EPSILON);
        assert!((left_up.tilt.0 + 0x493D as f32 / 208.0).abs() < f32::EPSILON);

        // Conversely, negative raw endpoints are right/down and map to
        // positive library degrees.
        let right_down =
            PanTiltPositionRaw::new(-0x08A58, -0x186A).as_degrees_with_profile(&SonyBRC300);
        assert!(right_down.pan.0 > 0.0);
        assert!(right_down.tilt.0 > 0.0);
        assert!((right_down.pan.0 - 0x08A58 as f32 / 208.0).abs() < f32::EPSILON);
        assert!((right_down.tilt.0 - 0x186A as f32 / 208.0).abs() < f32::EPSILON);

        let standard_only = PanTiltPositionRaw::new(0x08A58, 0x493D).as_degrees();
        assert!(standard_only.pan.0.is_nan());
        assert!(standard_only.tilt.0.is_nan());
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
            zoom.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)?
                .value(),
            0.0
        );
        assert_eq!(
            zoom.normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max)?
                .value(),
            0.0
        );

        // Test optical telephoto end
        let zoom = ZoomPosition::new(optical_max)?;
        assert_eq!(
            zoom.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)?
                .value(),
            1.0
        );
        assert!(
            (zoom
                .normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max)?
                .value()
                - 0.571)
                .abs()
                < 0.01
        );

        // Test digital telephoto end
        let zoom = ZoomPosition::new(0x7000)?;
        assert!(zoom
            .normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)
            .is_err());
        assert_eq!(
            zoom.normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max)?
                .value(),
            1.0
        );

        // Test middle position
        let zoom = ZoomPosition::new(0x2000)?;
        assert_eq!(
            zoom.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)?
                .value(),
            0.5
        );
        assert!(
            (zoom
                .normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max)?
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
    fn test_zoom_from_normalized_rejects_undocumented_digital_domain() -> Result<(), crate::Error> {
        let result = zoom_from_normalized(
            UnitInterval::ONE,
            ZoomDomain::OpticalPlusDigital,
            0x4000,
            None,
        );

        assert!(matches!(
            result,
            Err(crate::Error::FeatureNotSupported {
                feature: "optical-plus-digital zoom range"
            })
        ));
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
