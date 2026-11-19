//! Zoom normalization domains.

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
mod tests {
    use super::ZoomDomain;
    use crate::{
        capabilities::{CapabilityRange, ZoomCaps},
        profiles::ProfileId,
        types::ZoomPosition,
        units::UnitInterval,
        Error,
    };

    #[test]
    fn built_in_zoom_normalization_matches_the_selected_capability_range() -> crate::Result<()> {
        let inputs = [
            UnitInterval::MIN,
            UnitInterval::new(f32::from_bits(0x3848_3639))?,
            UnitInterval::new(0.5)?,
            UnitInterval::MAX,
        ];
        for &id in ProfileId::all() {
            let profile = id.spec()?;
            let Some(zoom) = &profile.capabilities().zoom else {
                continue;
            };
            for (domain, maximum) in [
                (ZoomDomain::Optical, Some(zoom.optical_max)),
                (ZoomDomain::OpticalPlusDigital, zoom.digital_max),
            ] {
                let Some(maximum) = maximum else {
                    for result in [
                        zoom.position_at(UnitInterval::MAX, domain).map(|_| ()),
                        zoom.normalize(ZoomPosition::MIN, domain).map(|_| ()),
                    ] {
                        assert!(
                            matches!(
                                result,
                                Err(Error::FeatureNotSupported {
                                    feature: "optical-plus-digital zoom range",
                                })
                            ),
                            "{id:?} {domain:?}: {result:?}"
                        );
                    }
                    continue;
                };
                let range = CapabilityRange::<u16>::new(0, maximum)?;
                for input in inputs {
                    let position = zoom.position_at(input, domain)?;
                    assert_eq!(
                        position.value(),
                        range.denormalize(input),
                        "{id:?} {domain:?}"
                    );
                    assert_eq!(
                        zoom.normalize(position, domain)?,
                        range.normalize(position.value())?,
                        "{id:?} {domain:?}",
                    );
                }
                assert_eq!(
                    zoom.normalize(ZoomPosition::MIN, domain)?,
                    UnitInterval::MIN
                );
                if maximum != 0 {
                    assert_eq!(
                        zoom.normalize(ZoomPosition::new(maximum)?, domain)?,
                        UnitInterval::MAX
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn normalized_zoom_rounds_at_the_actual_half_step() -> crate::Result<()> {
        let profile = ProfileId::SonyEviH100.spec()?;
        let zoom = profile
            .capabilities()
            .zoom
            .as_ref()
            .ok_or(Error::FeatureNotSupported { feature: "zoom" })?;
        for (bits, expected) in [(0x3848_3639, 1), (0x3848_363a, 2)] {
            let input = UnitInterval::new(f32::from_bits(bits))?;
            let position = zoom.position_at(input, ZoomDomain::OpticalPlusDigital)?;
            assert_eq!(position.value(), expected);
        }

        let zoom = ZoomCaps {
            optical_max: 3,
            digital_max: None,
            ..*zoom
        };
        let tie = 0.5_f32.to_bits();
        for (bits, expected) in [(tie - 1, 1), (tie, 2), (tie + 1, 2)] {
            let input = UnitInterval::new(f32::from_bits(bits))?;
            let position = zoom.position_at(input, ZoomDomain::Optical)?;
            assert_eq!(position.value(), expected);
        }
        Ok(())
    }

    #[test]
    fn singleton_zoom_domain_normalizes_to_zero_and_rejects_other_positions() -> crate::Result<()> {
        let zoom = ZoomCaps {
            optical_max: 0,
            digital_max: None,
            speed: CapabilityRange::<u8>::new(0, 0)?,
            direct: true,
            variable: false,
            optical_ratio: None,
        };
        let domain = ZoomDomain::Optical;
        for input in [
            UnitInterval::MIN,
            UnitInterval::new(0.5)?,
            UnitInterval::MAX,
        ] {
            assert_eq!(zoom.position_at(input, domain)?, ZoomPosition::MIN);
        }
        assert_eq!(
            zoom.normalize(ZoomPosition::MIN, domain)?,
            UnitInterval::MIN
        );
        assert!(matches!(
            zoom.normalize(ZoomPosition::new(1)?, domain),
            Err(Error::ParameterOutOfRange {
                parameter: "capability value",
                value: 1,
                min: 0,
                max: 0,
            })
        ));
        Ok(())
    }

    #[test]
    fn test_unit_interval_validation() -> crate::Result<()> {
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
