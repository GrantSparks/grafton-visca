//! Normalization domain and private profile-lowering helpers.

use crate::{types::ZoomPosition, units::UnitInterval};

/// Zoom domain for normalization.
///
/// Determines how zoom values are normalized to 0.0-1.0 range.
/// The actual max values are profile-specific (e.g., 0x4000 optical for
/// PTZOptics raw VISCA profiles, higher endpoints only for profiles with
/// validated digital zoom ranges).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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

#[cfg(test)]
impl ZoomPosition {
    /// Normalizes the zoom position to `0.0..=1.0` within a profile's zoom
    /// domain; [`zoom_from_normalized`] is the inverse.
    ///
    /// # Parameters
    /// - `domain`: whether to normalize against the optical or full zoom range
    /// - `optical_max`: the profile's maximum optical zoom value
    /// - `digital_max`: the profile's maximum digital zoom value, if supported
    ///
    /// # Errors
    /// Returns an error if the requested domain has no documented maximum, or
    /// if the raw position exceeds the selected domain maximum.
    pub(crate) fn normalize_with_max(
        &self,
        domain: ZoomDomain,
        optical_max: u16,
        digital_max: Option<u16>,
    ) -> Result<UnitInterval, crate::Error> {
        let max = zoom_domain_max(domain, optical_max, digital_max)?;

        if max == 0 {
            return Ok(UnitInterval::MIN);
        }

        if self.value() > max {
            return Err(crate::Error::parameter_out_of_range(
                "zoom position",
                i32::from(self.value()),
                0,
                i32::from(max),
            ));
        }

        UnitInterval::new(f32::from(self.value()) / f32::from(max))
    }
}

/// Returns the documented raw zoom maximum for a normalization domain.
///
/// Optical normalization always uses the profile optical maximum. Digital-domain
/// normalization requires an explicit digital maximum and never falls back to
/// the optical range.
pub(crate) fn zoom_domain_max(
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

/// Creates a zoom position from a normalized value within a profile's zoom
/// domain. Dividing a raw position by that domain's maximum is the inverse.
///
/// Like every unit conversion in the crate, the arithmetic is `f32`, rounding
/// to the nearest unit with halves away from zero.
pub(crate) fn zoom_from_normalized(
    normalized: UnitInterval,
    domain: ZoomDomain,
    optical_max: u16,
    digital_max: Option<u16>,
) -> Result<ZoomPosition, crate::Error> {
    let max = zoom_domain_max(domain, optical_max, digital_max)?;

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let raw_value = (normalized.value() * f32::from(max)).round() as u16;
    ZoomPosition::new(raw_value)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(matches!(
            zoom.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max),
            Err(crate::Error::ParameterOutOfRange {
                parameter: "zoom position",
                value: 0x7000,
                min: 0,
                max,
            }) if max == i32::from(optical_max)
        ));
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
            UnitInterval::MAX,
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
        assert_eq!(norm.value(), 0.75);
        Ok(())
    }
}
