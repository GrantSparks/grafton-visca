//! Optical magnification conversion from a profile's lens facts.

use std::borrow::Cow;

/// Linear magnification scale over a camera's optical zoom range.
///
/// Raw position `0` is 1x and the optical maximum is the lens's optical
/// ratio; positions in between are interpolated linearly. This is the crate's
/// only magnification ↔ unit conversion. Real lens curves are not linear, so
/// intermediate positions are approximate while both ends are exact.
/// Magnifications beyond the optical ratio (digital zoom) have no sourced
/// scale and are rejected. Arithmetic is `f32`, rounding to the nearest unit
/// with halves away from zero, like the pan/tilt conversion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomScale {
    optical_max: u16,
    optical_ratio: f32,
}

impl ZoomScale {
    /// Creates the scale for an optical range `0..=optical_max` and a lens
    /// with `optical_ratio`× optical zoom. The ratio maps exactly to
    /// `optical_max`, so the lens's nominal magnification is always reachable.
    /// Profile-backed callers use
    /// [`Capabilities::zoom_scale`](crate::capabilities::Capabilities::zoom_scale)
    /// or
    /// [`Capabilities::zoom_scale_for_lens`](crate::capabilities::Capabilities::zoom_scale_for_lens).
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidParameter`] if `optical_max` is zero or
    /// `optical_ratio` is not finite and greater than `1.0`.
    pub fn new(optical_max: u16, optical_ratio: f32) -> Result<Self, crate::Error> {
        if optical_max == 0 {
            return Err(crate::Error::InvalidParameter {
                parameter: "optical zoom maximum",
                value: Cow::Borrowed("0"),
                reason: Cow::Borrowed("an optical zoom range must be nonempty"),
            });
        }
        if !optical_ratio.is_finite() || optical_ratio <= 1.0 {
            return Err(crate::Error::InvalidParameter {
                parameter: "optical zoom ratio",
                value: Cow::Owned(optical_ratio.to_string()),
                reason: Cow::Borrowed("value must be finite and greater than 1.0"),
            });
        }
        Ok(Self {
            optical_max,
            optical_ratio,
        })
    }

    /// Returns the raw optical zoom maximum.
    #[must_use]
    pub const fn optical_max(self) -> u16 {
        self.optical_max
    }

    /// Returns the lens's optical zoom ratio.
    #[must_use]
    pub const fn optical_ratio(self) -> f32 {
        self.optical_ratio
    }

    /// Converts a magnification (`5.0` for 5x) to raw zoom units.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidParameter`] if `magnification` is not
    /// finite or is outside `1.0..=optical_ratio`.
    pub fn units(self, magnification: f32) -> Result<u16, crate::Error> {
        if !magnification.is_finite() || !(1.0..=self.optical_ratio).contains(&magnification) {
            return Err(crate::Error::InvalidParameter {
                parameter: "zoom magnification",
                value: Cow::Owned(magnification.to_string()),
                reason: Cow::Owned(format!(
                    "value must be finite and within 1.0..={}x optical",
                    self.optical_ratio
                )),
            });
        }
        let units = ((magnification - 1.0) / (self.optical_ratio - 1.0)
            * f32::from(self.optical_max))
        .round();
        // `units` lies in `0.0..=optical_max` because the magnification does.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok((units as u16).min(self.optical_max))
    }

    /// Converts raw zoom units to a magnification, or `None` for a position
    /// in the digital range beyond the optical maximum.
    #[must_use]
    pub fn magnification(self, units: u16) -> Option<f32> {
        (units <= self.optical_max).then(|| {
            1.0 + f32::from(units) / f32::from(self.optical_max) * (self.optical_ratio - 1.0)
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::ZoomScale;

    #[test]
    fn optical_scale_pins_endpoints_and_rejects_unsupported_magnification() {
        let scale = ZoomScale::new(0x4000, 20.0).expect("valid lens scale");
        assert_eq!(scale.units(1.0).ok(), Some(0));
        assert_eq!(scale.units(20.0).ok(), Some(0x4000));
        assert_eq!(scale.magnification(0), Some(1.0));
        assert_eq!(scale.magnification(0x4000), Some(20.0));
        assert_eq!(scale.magnification(0x4001), None);
        for invalid in [0.5, 20.01, f32::NAN, f32::INFINITY] {
            assert!(scale.units(invalid).is_err());
        }
        for ratio in [0.0, 1.0, f32::NAN, f32::INFINITY] {
            assert!(ZoomScale::new(0x4000, ratio).is_err());
        }
        assert!(ZoomScale::new(0, 20.0).is_err());
    }

    #[test]
    fn every_optical_unit_round_trips_for_supported_lens_ratios() {
        for ratio in [12.0, 20.0, 30.0] {
            let scale = ZoomScale::new(0x4000, ratio).expect("valid lens scale");
            for units in 0..=0x4000 {
                let magnification = scale.magnification(units).expect("optical unit");
                assert_eq!(scale.units(magnification).ok(), Some(units));
            }
        }
    }
}
