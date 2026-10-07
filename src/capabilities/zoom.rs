//! Zoom capability trait and associated types.

use std::borrow::Cow;

use crate::capabilities::{CapabilityRange, ValidationError};

/// Trait for cameras that support zoom operations.
///
/// This trait defines the constants and capabilities for zoom control.
/// Cameras implementing this trait gain access to zoom methods.
pub trait Zoom {
    /// Maximum optical zoom position in VISCA units (`0x4000` on the
    /// PTZOptics G2 family).
    const OPTICAL_ZOOM_MAX: u16;

    /// Maximum digital zoom position if supported.
    /// None if camera doesn't support digital zoom.
    const DIGITAL_ZOOM_MAX: Option<u16>;

    /// Valid range for variable zoom speed.
    /// Usually 0-7 where 0 is slowest, 7 is fastest.
    const ZOOM_SPEED_RANGE: CapabilityRange<u8>;

    /// Whether camera supports direct zoom positioning.
    /// If false, zoom must be achieved through zoom in/out commands.
    const SUPPORTS_DIRECT_ZOOM: bool = true;

    /// Whether camera supports variable speed zoom.
    /// If false, only standard speed zoom is available.
    const SUPPORTS_VARIABLE_ZOOM: bool = true;

    /// Optical zoom ratio of the profile's lens (`30.0` for a 30x lens).
    ///
    /// `Some` only when the profile names exactly one lens and a cited source
    /// states its ratio. A profile that spans several lenses (the PTZOptics G2
    /// family ships 12x, 20x and 30x models that answer VISCA identically)
    /// or whose ratio is unsourced is `None`. There are two declaration paths:
    /// a lens-dependent built-in profile names the installed lens per camera
    /// with
    /// [`Capabilities::zoom_scale_for_lens`](crate::capabilities::Capabilities::zoom_scale_for_lens),
    /// and a custom profile with a fixed lens states it once through the
    /// `optical_zoom_ratio` argument of
    /// [`ProfileSpecBuilder::zoom`](crate::ProfileSpecBuilder::zoom).
    const OPTICAL_ZOOM_RATIO: Option<f32>;
}

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

/// Extension trait that adds validation methods to cameras with zoom support.
pub trait ZoomExt: Zoom {
    /// Validate a zoom position is within range.
    fn validate_zoom_position(&self, position: u16) -> Result<u16, ValidationError> {
        let max = Self::DIGITAL_ZOOM_MAX.unwrap_or(Self::OPTICAL_ZOOM_MAX);
        CapabilityRange::<u16>::new(0, max).validate("zoom position", position)
    }

    /// Validate a zoom speed against `ZOOM_SPEED_RANGE`.
    ///
    /// Rejects out-of-range speeds instead of clamping them, matching the
    /// request path (`Capabilities::zoom_speed`).
    fn validate_zoom_speed(&self, speed: u8) -> Result<u8, ValidationError> {
        Self::ZOOM_SPEED_RANGE.validate("zoom speed", speed)
    }

    /// Check if position is in digital zoom range.
    fn is_digital_zoom(&self, position: u16) -> bool {
        position > Self::OPTICAL_ZOOM_MAX
    }
}

// Automatic implementation for all types that support zoom
impl<T: Zoom> ZoomExt for T {}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    struct TestCamera;

    impl Zoom for TestCamera {
        const OPTICAL_ZOOM_MAX: u16 = 0x4000;
        const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
        const ZOOM_SPEED_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(0, 7);
        const OPTICAL_ZOOM_RATIO: Option<f32> = Some(20.0);
    }

    #[test]
    fn test_zoom_validation() {
        let camera = TestCamera;

        assert!(camera.validate_zoom_position(0x4000).is_ok());
        assert!(camera.validate_zoom_position(0x7000).is_ok());
        assert!(camera.validate_zoom_position(0x8000).is_err());
    }

    #[test]
    fn zoom_speed_validation_rejects_instead_of_clamping() {
        let camera = TestCamera;

        assert_eq!(camera.validate_zoom_speed(0), Ok(0));
        assert_eq!(camera.validate_zoom_speed(7), Ok(7));
        assert_eq!(
            camera.validate_zoom_speed(8),
            Err(ValidationError::out_of_range("zoom speed", 8.0, 0.0, 7.0))
        );
    }

    #[test]
    fn test_digital_zoom_detection() {
        let camera = TestCamera;

        assert!(!camera.is_digital_zoom(0x4000));
        assert!(camera.is_digital_zoom(0x4001));
        assert!(camera.is_digital_zoom(0x7000));
    }

    #[test]
    fn zoom_scale_maps_both_optical_ends_exactly_and_rejects_digital_magnification() {
        let scale = ZoomScale::new(TestCamera::OPTICAL_ZOOM_MAX, 20.0).expect("valid lens");
        assert_eq!(scale.units(1.0).ok(), Some(0));
        assert_eq!(scale.units(20.0).ok(), Some(0x4000));
        assert_eq!(scale.magnification(0), Some(1.0));
        assert_eq!(scale.magnification(0x4000), Some(20.0));
        assert_eq!(scale.magnification(0x4001), None);
        for invalid in [0.5, 20.01, f32::NAN, f32::INFINITY] {
            assert!(matches!(
                scale.units(invalid),
                Err(crate::Error::InvalidParameter {
                    parameter: "zoom magnification",
                    ..
                })
            ));
        }
        for ratio in [1.0, 0.0, f32::NAN] {
            assert!(ZoomScale::new(0x4000, ratio).is_err());
        }
        assert!(ZoomScale::new(0, 20.0).is_err());
    }

    /// Every unit maps to a magnification that maps back to the same unit.
    #[test]
    fn zoom_scale_round_trips_every_optical_unit() {
        for ratio in [12.0, 20.0, 30.0] {
            let scale = ZoomScale::new(0x4000, ratio).expect("valid lens");
            for units in 0..=0x4000_u16 {
                let magnification = scale.magnification(units).expect("optical unit");
                assert_eq!(
                    scale.units(magnification).ok(),
                    Some(units),
                    "{ratio}x {units}"
                );
            }
        }
    }
}
