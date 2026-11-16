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
    /// Ordered focus position bounds, independent of physical orientation.
    const FOCUS_RANGE: CapabilityRange<u16>;

    /// The sourced near endpoint, or `Unknown` when raw orientation is unverified.
    const FOCUS_NEAR_END: crate::capabilities::FocusNearEnd;

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
    const MAX_FOCUS_SPEED: u8;

    /// Whether camera supports auto focus sensitivity adjustment.
    const SUPPORTS_AF_SENSITIVITY: bool = false;

    /// Whether camera supports the focus near limit inquiry command.
    /// Some cameras (e.g., PTZOptics G2) can accept the near limit set
    /// command but don't support reading it back via VISCA inquiry.
    const SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY: bool = false;
}

/// Extension trait that adds validation methods to cameras with focus support.
pub trait FocusExt: Focus {
    /// Validate a focus position is within range.
    ///
    /// `FOCUS_RANGE` is the checked numerical interval; it makes no claim
    /// about physical near and far orientation.
    fn validate_focus_position(&self, position: u16) -> Result<u16, ValidationError> {
        Self::FOCUS_RANGE.validate("focus position", position)
    }

    /// Validate a focus speed against `0..=MAX_FOCUS_SPEED`.
    ///
    /// Rejects out-of-range speeds instead of clamping them, matching the
    /// request path (`FocusCaps::speed`).
    fn validate_focus_speed(&self, speed: u8) -> Result<u8, ValidationError> {
        CapabilityRange::<u8>::new(0, Self::MAX_FOCUS_SPEED).validate("focus speed", speed)
    }

    /// Convert a normalized focus position to VISCA units.
    ///
    /// Zero is the near endpoint and one is the far endpoint when the source
    /// establishes their orientation.
    ///
    /// # Errors
    /// Returns [`ValidationError::NotSupported`] when `FOCUS_NEAR_END` is
    /// [`Unknown`](crate::capabilities::FocusNearEnd::Unknown). Numeric raw
    /// bounds alone do not establish a physical near-to-far direction.
    ///
    /// ```rust
    /// use grafton_visca::{
    ///     capabilities::{focus::{Focus, FocusExt}, ValidationError},
    ///     profiles::PtzOpticsG2,
    ///     types::UnitInterval,
    /// };
    /// let minimum = <PtzOpticsG2 as Focus>::FOCUS_RANGE.min();
    /// assert_eq!(PtzOpticsG2.validate_focus_position(minimum), Ok(minimum));
    /// assert_eq!(
    ///     PtzOpticsG2.normalized_to_focus_units(UnitInterval::MIN),
    ///     Err(ValidationError::NotSupported("focus near/far orientation")),
    /// );
    /// ```
    fn normalized_to_focus_units(&self, normalized: UnitInterval) -> Result<u16, ValidationError> {
        let range = Self::FOCUS_RANGE;
        let span = range.max() - range.min();
        let offset = (normalized.value() * f32::from(span)).round() as u16;
        let position = match Self::FOCUS_NEAR_END {
            crate::capabilities::FocusNearEnd::Unknown => {
                return Err(ValidationError::NotSupported("focus near/far orientation"))
            }
            crate::capabilities::FocusNearEnd::Minimum => range.min() + offset,
            crate::capabilities::FocusNearEnd::Maximum => range.max() - offset,
        };
        self.validate_focus_position(position)
    }

    /// Converts raw units to a fraction from the near endpoint to the far end.
    ///
    /// # Errors
    /// Rejects positions outside `FOCUS_RANGE`, or returns
    /// [`ValidationError::NotSupported`] when physical orientation is unknown.
    fn focus_units_to_normalized(&self, units: u16) -> Result<UnitInterval, ValidationError> {
        self.validate_focus_position(units)?;
        let range = Self::FOCUS_RANGE;
        let offset = match Self::FOCUS_NEAR_END {
            crate::capabilities::FocusNearEnd::Unknown => {
                return Err(ValidationError::NotSupported("focus near/far orientation"))
            }
            crate::capabilities::FocusNearEnd::Minimum => units - range.min(),
            crate::capabilities::FocusNearEnd::Maximum => range.max() - units,
        };
        UnitInterval::new(f32::from(offset) / f32::from(range.max() - range.min())).map_err(|_| {
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
        const SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY: bool = true;

        const MAX_FOCUS_SPEED: u8 = 7;

        const FOCUS_RANGE: CapabilityRange<u16> = CapabilityRange::<u16>::new(0x1000, 0xF000);
        const FOCUS_NEAR_END: crate::capabilities::FocusNearEnd =
            crate::capabilities::FocusNearEnd::Minimum;
        const SUPPORTS_AUTO_FOCUS: bool = true;
        const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
    }

    #[test]
    fn omitted_focus_near_limit_inquiry_is_conservative() {
        struct Conservative;
        impl Focus for Conservative {
            const FOCUS_RANGE: CapabilityRange<u16> = CapabilityRange::<u16>::new(0, u16::MAX);
            const FOCUS_NEAR_END: crate::capabilities::FocusNearEnd =
                crate::capabilities::FocusNearEnd::Minimum;
            const SUPPORTS_AUTO_FOCUS: bool = false;
            const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
            const MAX_FOCUS_SPEED: u8 = 0;
        }
        const { assert!(!<Conservative as Focus>::SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY) };
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn every_builtin_focus_position_round_trips_through_normalized_units() {
        fn check<P: Focus + Default>() {
            let camera = P::default();
            if matches!(
                P::FOCUS_NEAR_END,
                crate::capabilities::FocusNearEnd::Unknown
            ) {
                assert!(matches!(
                    camera.normalized_to_focus_units(UnitInterval::MIN),
                    Err(ValidationError::NotSupported("focus near/far orientation"))
                ));
                assert!(matches!(
                    camera.focus_units_to_normalized(P::FOCUS_RANGE.min()),
                    Err(ValidationError::NotSupported("focus near/far orientation"))
                ));
                assert!(camera.validate_focus_position(P::FOCUS_RANGE.min()).is_ok());
                return;
            }
            for position in P::FOCUS_RANGE.min()..=P::FOCUS_RANGE.max() {
                let normalized = camera.focus_units_to_normalized(position).unwrap();
                assert_eq!(
                    camera.normalized_to_focus_units(normalized).unwrap(),
                    position,
                    "{} focus position {position:#06x}",
                    core::any::type_name::<P>()
                );
            }
        }
        check::<crate::profiles::GenericVisca>();
        check::<crate::profiles::PtzOpticsG2>();
        check::<crate::profiles::PtzOpticsG3>();
        check::<crate::profiles::PtzOptics30X>();
        check::<crate::profiles::SonyFR7>();
        check::<crate::profiles::SonyBRCH900>();
        check::<crate::profiles::SonyEVIH100>();
        check::<crate::profiles::SonyBRC300>();
        check::<crate::profiles::NearusBRC300>();
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
                .normalized_to_focus_units(UnitInterval::MIN)
                .expect("0.0 is valid normalized value"),
            0x1000
        );
        assert_eq!(
            camera
                .normalized_to_focus_units(UnitInterval::MAX)
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
