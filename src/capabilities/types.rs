//! Supporting types used across capability traits.

use std::{borrow::Cow, ops::RangeInclusive};

use crate::capabilities::ValidationError;

// Re-export types that are used by multiple capability traits
pub use crate::capabilities::exposure::ShutterSpeedEntry;

/// Inclusive numeric bounds for profile capability metadata.
///
/// VISCA profile facts are closed ranges: both endpoints are supported values.
/// This type keeps those protocol facts explicit in profile traits and registry
/// literals while still exposing standard [`RangeInclusive`] values for runtime
/// discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CapabilityRange<T> {
    min: T,
    max: T,
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for CapabilityRange<T>
where
    T: serde::Deserialize<'de> + PartialOrd,
{
    fn deserialize<__D>(deserializer: __D) -> Result<Self, __D::Error>
    where
        __D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct WireRange<T> {
            min: T,
            max: T,
        }

        let wire = WireRange::<T>::deserialize(deserializer)?;
        if wire.min <= wire.max {
            Ok(Self {
                min: wire.min,
                max: wire.max,
            })
        } else {
            Err(serde::de::Error::custom(
                "capability range minimum exceeds maximum",
            ))
        }
    }
}

/// The positions a profile admits for one numeric fact whose source is a
/// table: closed bounds minus the positions the table does not list.
///
/// Both bounds are always admitted. A gap is a value strictly between them
/// that the source documents no meaning for, such as the unlisted iris and
/// bright positions `01`..`04` of the Sony EVI-H100 tables. Construction and
/// deserialization keep the gaps sorted, unique, and strictly inside the
/// bounds, so every value is a well-formed domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CapabilityDomain<T: Clone + 'static> {
    min: T,
    max: T,
    gaps: Cow<'static, [T]>,
}

/// Why a value inside a [`CapabilityDomain`]'s bounds is refused. Both
/// validation vocabularies report a gap with this text.
pub(crate) const DOMAIN_GAP: &str = "the profile's source table does not list this position";

/// Where one value falls in a [`CapabilityDomain`]: the single admission rule
/// that both validation vocabularies map from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DomainAdmission {
    /// The value is a documented position.
    Admitted,
    /// The value is outside the bounds.
    OutOfBounds,
    /// The value is inside the bounds but the source does not list it.
    Gap,
}

impl<T> CapabilityDomain<T>
where
    T: Copy + PartialOrd + 'static,
{
    /// Returns the inclusive minimum, which is always admitted.
    #[must_use]
    pub const fn min(&self) -> T {
        self.min
    }

    /// Returns the inclusive maximum, which is always admitted.
    #[must_use]
    pub const fn max(&self) -> T {
        self.max
    }

    /// Returns the unlisted positions strictly inside the bounds, in
    /// ascending order.
    #[must_use]
    pub fn gaps(&self) -> &[T] {
        &self.gaps
    }

    /// Returns the bounds as a standard inclusive range. Gaps are not
    /// represented; use [`Self::contains`] to test a value.
    #[must_use]
    pub fn bounds(&self) -> RangeInclusive<T> {
        self.min..=self.max
    }

    /// Returns true when `value` is a documented position.
    #[must_use]
    pub fn contains(&self, value: T) -> bool {
        self.admission(value) == DomainAdmission::Admitted
    }

    pub(crate) fn admission(&self, value: T) -> DomainAdmission {
        if value < self.min || value > self.max {
            DomainAdmission::OutOfBounds
        } else if self.gaps.contains(&value) {
            DomainAdmission::Gap
        } else {
            DomainAdmission::Admitted
        }
    }
}

macro_rules! impl_capability_domain {
    ($ty:ty) => {
        impl CapabilityDomain<$ty> {
            /// Creates a domain with no gaps.
            ///
            /// # Panics
            /// Panics when `min > max`.
            #[must_use]
            pub const fn new(min: $ty, max: $ty) -> Self {
                Self::with_gaps(min, max, &[])
            }

            /// Creates a domain whose source table does not list `gaps`.
            ///
            /// # Panics
            /// Panics when `min > max`, or when `gaps` is not sorted, unique,
            /// and strictly inside the bounds. In a `const` item that is a
            /// compile error.
            #[must_use]
            pub const fn with_gaps(min: $ty, max: $ty, gaps: &'static [$ty]) -> Self {
                assert!(
                    Self::is_well_formed(min, max, gaps),
                    "capability domain bounds must be ordered and its gaps sorted, unique, \
                     and strictly inside them"
                );
                Self {
                    min,
                    max,
                    gaps: Cow::Borrowed(gaps),
                }
            }

            /// The invariant every domain holds: ordered bounds, and gaps
            /// sorted, unique, and strictly inside them.
            const fn is_well_formed(min: $ty, max: $ty, gaps: &[$ty]) -> bool {
                if min > max {
                    return false;
                }
                let mut index = 0;
                while index < gaps.len() {
                    if gaps[index] <= min
                        || gaps[index] >= max
                        || (index > 0 && gaps[index - 1] >= gaps[index])
                    {
                        return false;
                    }
                    index += 1;
                }
                true
            }

            /// Checks `value` against the domain.
            ///
            /// This is the same admission rule request preparation applies.
            ///
            /// # Errors
            /// [`ValidationError::OutOfRange`] naming `parameter` when `value`
            /// is outside the bounds, and [`ValidationError::InvalidValue`]
            /// when it is a gap.
            pub fn validate(
                &self,
                parameter: &'static str,
                value: $ty,
            ) -> Result<$ty, ValidationError> {
                match self.admission(value) {
                    DomainAdmission::Admitted => Ok(value),
                    DomainAdmission::OutOfBounds => {
                        CapabilityRange::<$ty>::new(self.min, self.max).validate(parameter, value)
                    }
                    DomainAdmission::Gap => {
                        Err(ValidationError::invalid_value(parameter, DOMAIN_GAP))
                    }
                }
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for CapabilityDomain<$ty> {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                #[derive(serde::Deserialize)]
                struct WireDomain {
                    min: $ty,
                    max: $ty,
                    gaps: Vec<$ty>,
                }

                let wire = WireDomain::deserialize(deserializer)?;
                if !Self::is_well_formed(wire.min, wire.max, &wire.gaps) {
                    return Err(serde::de::Error::custom(
                        "capability domain bounds must be ordered and its gaps sorted, unique, \
                         and strictly inside them",
                    ));
                }
                Ok(Self {
                    min: wire.min,
                    max: wire.max,
                    gaps: Cow::Owned(wire.gaps),
                })
            }
        }

        impl SupportedRange<$ty> for Option<CapabilityDomain<$ty>> {
            fn validate_supported(
                self,
                parameter: &'static str,
                value: $ty,
            ) -> Result<$ty, ValidationError> {
                match self {
                    Some(domain) => domain.validate(parameter, value),
                    None => Err(ValidationError::NotSupported(parameter)),
                }
            }
        }
    };
}

impl_capability_domain!(u8);
impl_capability_domain!(u16);

/// Crate-private check of a value against a profile fact that may be absent.
///
/// Implemented for `Option<CapabilityRange<T>>` and
/// `Option<CapabilityDomain<T>>`, so every `*Ext` helper over an optional fact
/// shares one rule: `None` is unsupported, `Some` is checked with
/// [`CapabilityRange::validate`] or [`CapabilityDomain::validate`].
pub(crate) trait SupportedRange<T> {
    /// Returns `value` when the range exists and contains it.
    ///
    /// # Errors
    /// [`ValidationError::NotSupported`] when there is no range, and
    /// [`ValidationError::OutOfRange`] when `value` lies outside it.
    fn validate_supported(self, parameter: &'static str, value: T) -> Result<T, ValidationError>;
}

macro_rules! impl_capability_range {
    ($ty:ty) => {
        impl CapabilityRange<$ty> {
            /// Creates an inclusive capability range.
            ///
            /// # Panics
            /// Panics when `min > max`.
            #[must_use]
            pub const fn new(min: $ty, max: $ty) -> Self {
                assert!(min <= max, "capability range minimum exceeds maximum");
                Self { min, max }
            }

            /// Returns the inclusive minimum value.
            #[must_use]
            pub const fn min(self) -> $ty {
                self.min
            }

            /// Returns the inclusive maximum value.
            #[must_use]
            pub const fn max(self) -> $ty {
                self.max
            }

            /// Returns true when `value` is within the closed bounds.
            #[must_use]
            pub const fn contains(self, value: $ty) -> bool {
                value >= self.min && value <= self.max
            }

            /// Checks `value` against the closed bounds.
            ///
            /// This is the single range rule behind the `*Ext` validation
            /// helpers. It rejects out-of-range values and never clamps.
            ///
            /// # Errors
            /// [`ValidationError::OutOfRange`] naming `parameter` when `value`
            /// is outside the bounds.
            pub fn validate(
                self,
                parameter: &'static str,
                value: $ty,
            ) -> Result<$ty, ValidationError> {
                if self.contains(value) {
                    Ok(value)
                } else {
                    Err(ValidationError::out_of_range(
                        parameter,
                        f64::from(value),
                        f64::from(self.min),
                        f64::from(self.max),
                    ))
                }
            }

            /// Converts the closed bounds to a standard inclusive range.
            #[must_use]
            pub fn as_inclusive(self) -> RangeInclusive<$ty> {
                self.min..=self.max
            }
        }

        impl SupportedRange<$ty> for Option<CapabilityRange<$ty>> {
            fn validate_supported(
                self,
                parameter: &'static str,
                value: $ty,
            ) -> Result<$ty, ValidationError> {
                match self {
                    Some(range) => range.validate(parameter, value),
                    None => Err(ValidationError::NotSupported(parameter)),
                }
            }
        }
    };
}

impl_capability_range!(u8);
impl_capability_range!(u16);
impl_capability_range!(i8);
impl_capability_range!(i16);
impl_capability_range!(i32);

/// Coordinate system used by a camera for pan/tilt positions.
///
/// Different camera models use different coordinate representations:
/// - Modern cameras use signed coordinates centered at (0,0)
/// - Some cameras use unsigned coordinates with (0x8000,0x8000) as center
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum CoordinateSystem {
    /// Signed coordinates with (0,0) as the center position.
    /// Used by most modern cameras.
    SignedCentered,
    /// Unsigned coordinates with (0x8000,0x8000) as the center position.
    /// Used by cameras whose standard 16-bit fields are offset from center.
    UnsignedCentered,
}

impl CoordinateSystem {
    /// Convert logical coordinates (signed, centered at 0) to camera coordinates.
    #[must_use]
    pub fn to_camera_coords(self, pan: i16, tilt: i16) -> (u16, u16) {
        match self {
            Self::SignedCentered => {
                // Direct conversion, interpreting as unsigned
                (pan as u16, tilt as u16)
            }
            Self::UnsignedCentered => {
                // Add offset to center at 0x8000
                let pan_u16 = ((pan as i32) + 0x8000) as u16;
                let tilt_u16 = ((tilt as i32) + 0x8000) as u16;
                (pan_u16, tilt_u16)
            }
        }
    }

    /// Convert camera coordinates to logical coordinates (signed, centered at 0).
    #[must_use]
    pub fn convert_from_camera_coords(self, pan: u16, tilt: u16) -> (i16, i16) {
        match self {
            Self::SignedCentered => {
                // Direct conversion, interpreting as signed
                (pan as i16, tilt as i16)
            }
            Self::UnsignedCentered => {
                // Subtract offset to center at 0
                let pan_i16 = ((pan as i32) - 0x8000) as i16;
                let tilt_i16 = ((tilt as i32) - 0x8000) as i16;
                (pan_i16, tilt_i16)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_range_constructs_valid_signed_and_unsigned_ranges() {
        assert_eq!(CapabilityRange::<u8>::new(1_u8, 3).as_inclusive(), 1..=3);
        assert_eq!(
            CapabilityRange::<u16>::new(0_u16, 10).as_inclusive(),
            0..=10
        );
        assert_eq!(CapabilityRange::<i8>::new(-7_i8, 7).as_inclusive(), -7..=7);
        assert_eq!(
            CapabilityRange::<i16>::new(-170_i16, 170).as_inclusive(),
            -170..=170
        );
    }

    #[test]
    fn validate_accepts_bounds_and_rejects_outside_without_clamping() {
        let range = CapabilityRange::<u8>::new(1_u8, 3);
        assert_eq!(range.validate("speed", 1), Ok(1));
        assert_eq!(range.validate("speed", 3), Ok(3));
        assert_eq!(
            range.validate("speed", 0),
            Err(ValidationError::out_of_range("speed", 0.0, 1.0, 3.0))
        );
        assert_eq!(
            range.validate("speed", 4),
            Err(ValidationError::out_of_range("speed", 4.0, 1.0, 3.0))
        );
        let signed = CapabilityRange::<i16>::new(-20_i16, 20);
        assert_eq!(signed.validate("pos", -20), Ok(-20));
        assert_eq!(
            signed.validate("pos", -21),
            Err(ValidationError::out_of_range("pos", -21.0, -20.0, 20.0))
        );
        let wide = CapabilityRange::<i32>::new(-5_i32, i32::MAX);
        assert_eq!(wide.validate("w", i32::MAX), Ok(i32::MAX));
    }

    #[test]
    fn validate_supported_distinguishes_absent_from_out_of_range() {
        let none: Option<CapabilityRange<u16>> = None;
        assert_eq!(
            none.validate_supported("hue", 1),
            Err(ValidationError::NotSupported("hue"))
        );
        let some = Some(CapabilityRange::<u16>::new(0_u16, 14));
        assert_eq!(some.validate_supported("hue", 14), Ok(14));
        assert_eq!(
            some.validate_supported("hue", 15),
            Err(ValidationError::out_of_range("hue", 15.0, 0.0, 14.0))
        );
    }

    #[test]
    fn capability_domain_admits_bounds_and_refuses_gaps_through_one_rule() {
        let domain = CapabilityDomain::<u8>::with_gaps(0x00, 0x11, &[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(domain.gaps(), &[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(domain.bounds(), 0x00..=0x11);
        for admitted in [0x00, 0x05, 0x11] {
            assert!(domain.contains(admitted));
            assert_eq!(domain.validate("iris", admitted), Ok(admitted));
        }
        for gap in 0x01..=0x04 {
            assert_eq!(domain.admission(gap), DomainAdmission::Gap);
            assert!(matches!(
                domain.validate("iris", gap),
                Err(ValidationError::InvalidValue { .. })
            ));
        }
        assert_eq!(domain.admission(0x12), DomainAdmission::OutOfBounds);
        assert_eq!(
            domain.validate("iris", 0x12),
            Err(ValidationError::out_of_range("iris", 18.0, 0.0, 17.0))
        );
        assert_eq!(
            None::<CapabilityDomain<u8>>.validate_supported("iris", 0),
            Err(ValidationError::NotSupported("iris"))
        );
    }

    #[test]
    fn capability_domain_refuses_malformed_gaps() {
        for (min, max, gaps) in [
            (0_u16, 17, &[0_u16][..]),
            (0, 17, &[17]),
            (0, 17, &[4, 2]),
            (0, 17, &[2, 2]),
            (18, 17, &[]),
        ] {
            assert!(
                std::panic::catch_unwind(|| CapabilityDomain::<u16>::with_gaps(min, max, gaps))
                    .is_err(),
                "{min}..={max} {gaps:?}"
            );
        }
    }

    #[test]
    fn capability_range_rejects_inverted_ranges() {
        assert!(std::panic::catch_unwind(|| CapabilityRange::<u8>::new(2_u8, 1)).is_err());
        assert!(std::panic::catch_unwind(|| CapabilityRange::<u16>::new(2_u16, 1)).is_err());
        assert!(std::panic::catch_unwind(|| CapabilityRange::<i8>::new(2_i8, 1)).is_err());
        assert!(std::panic::catch_unwind(|| CapabilityRange::<i16>::new(2_i16, 1)).is_err());
    }

    #[test]
    fn capability_range_allows_single_value_ranges() {
        let range = CapabilityRange::<u8>::new(5_u8, 5);

        assert_eq!(range.min(), 5);
        assert_eq!(range.max(), 5);
        assert!(range.contains(5));
        assert!(!range.contains(4));
        assert_eq!(range.as_inclusive(), 5..=5);
    }

    #[test]
    fn capability_range_represents_full_width_u8_ranges() {
        let range = CapabilityRange::<u8>::new(0_u8, u8::MAX);

        assert!(range.contains(0));
        assert!(range.contains(u8::MAX));
        assert_eq!(range.as_inclusive(), 0..=u8::MAX);
    }

    #[test]
    fn capability_range_methods_work_for_supported_primitives() {
        let u8_range = CapabilityRange::<u8>::new(1_u8, 3);
        assert_eq!(u8_range.min(), 1);
        assert_eq!(u8_range.max(), 3);
        assert!(u8_range.contains(2));
        assert_eq!(u8_range.as_inclusive(), 1..=3);

        let u16_range = CapabilityRange::<u16>::new(10_u16, 12);
        assert_eq!(u16_range.min(), 10);
        assert_eq!(u16_range.max(), 12);
        assert!(u16_range.contains(11));
        assert_eq!(u16_range.as_inclusive(), 10..=12);

        let i8_range = CapabilityRange::<i8>::new(-2_i8, 2);
        assert_eq!(i8_range.min(), -2);
        assert_eq!(i8_range.max(), 2);
        assert!(i8_range.contains(0));
        assert_eq!(i8_range.as_inclusive(), -2..=2);

        let i16_range = CapabilityRange::<i16>::new(-20_i16, 20);
        assert_eq!(i16_range.min(), -20);
        assert_eq!(i16_range.max(), 20);
        assert!(i16_range.contains(0));
        assert_eq!(i16_range.as_inclusive(), -20..=20);
    }

    #[test]
    fn test_signed_centered_coordinates() {
        let coord_system = CoordinateSystem::SignedCentered;

        // Test conversion to camera coords
        let (cam_pan, cam_tilt) = coord_system.to_camera_coords(1000, -500);
        assert_eq!(cam_pan, 1000_u16);
        assert_eq!(cam_tilt, (-500_i16) as u16);

        // Test conversion from camera coords
        let (log_pan, log_tilt) = coord_system.convert_from_camera_coords(1000, (-500_i16) as u16);
        assert_eq!(log_pan, 1000);
        assert_eq!(log_tilt, -500);
    }

    #[test]
    fn test_unsigned_centered_coordinates() {
        let coord_system = CoordinateSystem::UnsignedCentered;

        // Test conversion to camera coords (add 0x8000)
        let (cam_pan, cam_tilt) = coord_system.to_camera_coords(1000, -500);
        assert_eq!(cam_pan, 0x8000 + 1000);
        assert_eq!(cam_tilt, 0x8000 - 500);

        // Test conversion from camera coords (subtract 0x8000)
        let (log_pan, log_tilt) =
            coord_system.convert_from_camera_coords(0x8000 + 1000, 0x8000 - 500);
        assert_eq!(log_pan, 1000);
        assert_eq!(log_tilt, -500);

        // Test center position
        let (cam_pan, cam_tilt) = coord_system.to_camera_coords(0, 0);
        assert_eq!(cam_pan, 0x8000);
        assert_eq!(cam_tilt, 0x8000);

        let (log_pan, log_tilt) = coord_system.convert_from_camera_coords(0x8000, 0x8000);
        assert_eq!(log_pan, 0);
        assert_eq!(log_tilt, 0);
    }
}
