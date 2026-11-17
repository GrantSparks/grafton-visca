//! Supporting types used across capability traits.

use std::{borrow::Cow, ops::RangeInclusive};

use crate::Error;

/// Inclusive numeric bounds for profile capability metadata.
///
/// VISCA profile facts are closed ranges: both endpoints are supported values.
/// This type keeps those protocol facts explicit in profile traits and registry
/// literals while still exposing standard [`RangeInclusive`] values for runtime
/// discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
        #[serde(deny_unknown_fields)]
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

impl<T: Copy> CapabilityRange<T> {
    /// Returns the inclusive minimum.
    #[must_use]
    pub const fn min(self) -> T {
        self.min
    }

    /// Returns the inclusive maximum.
    #[must_use]
    pub const fn max(self) -> T {
        self.max
    }

    /// Projects the validated closed bounds to a standard inclusive interval.
    #[must_use]
    pub fn as_inclusive(self) -> RangeInclusive<T> {
        self.min..=self.max
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
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct CapabilityDomain<T: Clone + 'static> {
    min: T,
    max: T,
    gaps: Cow<'static, [T]>,
}

/// Why a value inside a [`CapabilityDomain`]'s bounds is refused. Request preparation and public validation
/// report a gap with this text.
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

    /// Returns the validated inclusive bounds. Gaps are not
    /// represented; use [`Self::contains`] to test a value.
    #[must_use]
    pub fn bounds(&self) -> CapabilityRange<T> {
        CapabilityRange {
            min: self.min,
            max: self.max,
        }
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
            /// [`Error::ParameterOutOfRange`] naming `parameter` when `value`
            /// is outside the bounds, and [`Error::InvalidParameter`]
            /// when it is a gap.
            pub fn validate(&self, parameter: &'static str, value: $ty) -> crate::Result<$ty> {
                match self.admission(value) {
                    DomainAdmission::Admitted => Ok(value),
                    DomainAdmission::OutOfBounds => {
                        CapabilityRange::<$ty>::new(self.min, self.max).validate(parameter, value)
                    }
                    DomainAdmission::Gap => Err(Error::invalid_parameter(
                        parameter,
                        value.to_string(),
                        DOMAIN_GAP,
                    )),
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
                #[serde(deny_unknown_fields)]
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
    };
}

impl_capability_domain!(u8);
impl_capability_domain!(u16);

macro_rules! impl_capability_range {
    ($ty:ty) => {
        impl CapabilityRange<$ty> {
            /// Normalizes a supported value within this profile's bounds.
            ///
            /// A single-value domain normalizes to zero. Values outside the
            /// declared bounds are rejected before conversion.
            #[allow(clippy::cast_precision_loss)]
            pub fn normalize(self, value: $ty) -> crate::Result<crate::types::UnitInterval> {
                self.validate("capability value", value)?;
                if value == self.min {
                    return Ok(crate::types::UnitInterval::MIN);
                }
                if value == self.max {
                    return Ok(crate::types::UnitInterval::MAX);
                }
                let offset = i64::from(value) - i64::from(self.min);
                let span = i64::from(self.max) - i64::from(self.min);
                crate::types::UnitInterval::new(offset as f32 / span as f32)
            }

            /// Maps a normalized value into this profile's inclusive domain.
            ///
            /// A single-value domain always returns its sole value; normalized
            /// zero and one return the exact endpoints. Interpolation uses
            /// `f64`, which represents every supported integer endpoint and
            /// span exactly, and rounds to the nearest integer with halves
            /// away from zero. The result stays within the declared bounds.
            #[must_use]
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            pub fn denormalize(self, value: crate::types::UnitInterval) -> $ty {
                if self.min == self.max || value == crate::types::UnitInterval::MIN {
                    return self.min;
                }
                if value == crate::types::UnitInterval::MAX {
                    return self.max;
                }
                let min = f64::from(self.min);
                let span = f64::from(self.max) - min;
                (min + f64::from(value.value()) * span).round() as $ty
            }

            /// Creates an inclusive capability range.
            ///
            /// # Panics
            /// Panics when `min > max`.
            #[must_use]
            pub const fn new(min: $ty, max: $ty) -> Self {
                assert!(min <= max, "capability range minimum exceeds maximum");
                Self { min, max }
            }

            /// Returns true when `value` is within the closed bounds.
            #[must_use]
            pub const fn contains(self, value: $ty) -> bool {
                value >= self.min && value <= self.max
            }

            /// Checks `value` against the closed bounds.
            ///
            /// It rejects out-of-range values and never clamps.
            ///
            /// # Errors
            /// [`Error::ParameterOutOfRange`] naming `parameter` when `value`
            /// is outside the bounds.
            pub fn validate(self, parameter: &'static str, value: $ty) -> crate::Result<$ty> {
                if self.contains(value) {
                    Ok(value)
                } else {
                    Err(Error::parameter_out_of_range(
                        parameter,
                        i32::from(value),
                        i32::from(self.min),
                        i32::from(self.max),
                    ))
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
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
    use proptest::prelude::*;

    use super::*;
    use crate::types::UnitInterval;

    #[allow(clippy::expect_used)]
    fn unit_interval(value: f32) -> UnitInterval {
        UnitInterval::new(value).expect("test input is finite and within zero through one")
    }

    #[test]
    fn denormalize_preserves_i32_values_beyond_f32_integer_precision() {
        for (min, max, normalized, expected) in [
            (16_777_217, 16_777_217, 0.5, 16_777_217),
            (16_777_217, 16_777_220, 0.01, 16_777_217),
            (-16_777_217, -16_777_217, 0.5, -16_777_217),
            (-16_777_220, -16_777_217, 0.99, -16_777_217),
            (i32::MIN, i32::MIN, 0.5, i32::MIN),
            (i32::MAX, i32::MAX, 0.5, i32::MAX),
            (i32::MIN, i32::MIN + 3, 0.5, i32::MIN + 1),
            (i32::MAX - 3, i32::MAX, 0.5, i32::MAX - 1),
        ] {
            let range = CapabilityRange::<i32>::new(min, max);
            let actual = range.denormalize(unit_interval(normalized));
            assert!(
                range.contains(actual),
                "returned {actual} outside {range:?}"
            );
            assert_eq!(actual, expected, "{range:?} at {normalized}");
        }
    }

    #[test]
    fn denormalize_rounds_signed_halfway_coordinates_away_from_zero() {
        for (min, max, expected) in [
            (-2, -1, -2),
            (-1, 0, -1),
            (0, 1, 1),
            (1, 2, 2),
            (-2, 1, -1),
            (-1, 2, 1),
            (i32::MIN, i32::MAX, -1),
            (i32::MIN + 1, i32::MAX, 0),
        ] {
            let range = CapabilityRange::<i32>::new(min, max);
            assert_eq!(
                range.denormalize(unit_interval(0.5)),
                expected,
                "{range:?} at 0.5"
            );
        }
    }

    // Eighths have exact binary representations. This integer oracle rounds
    // the rational coordinate independently of floating-point interpolation.
    macro_rules! assert_interpolation_at_eighths {
        ($ty:ty, $endpoints:expr) => {
            for min in $endpoints {
                for max in $endpoints {
                    if min > max {
                        continue;
                    }
                    let range = CapabilityRange::<$ty>::new(min, max);
                    for step in 0_u8..=8 {
                        let normalized = unit_interval(f32::from(step) / 8.0);
                        let numerator =
                            i64::from(min) * i64::from(8 - step) + i64::from(max) * i64::from(step);
                        let magnitude = (numerator.abs() + 4) / 8;
                        let expected = if numerator < 0 { -magnitude } else { magnitude };
                        assert_eq!(
                            i64::from(range.denormalize(normalized)),
                            expected,
                            "{range:?} at {step}/8"
                        );
                    }
                }
            }
        };
    }

    #[test]
    fn denormalize_matches_rational_oracle_for_every_u8_range() {
        assert_interpolation_at_eighths!(u8, u8::MIN..=u8::MAX);
    }

    #[test]
    fn denormalize_matches_rational_oracle_for_every_i8_range() {
        assert_interpolation_at_eighths!(i8, i8::MIN..=i8::MAX);
    }

    #[test]
    fn denormalize_matches_rational_oracle_for_wider_integer_boundaries() {
        assert_interpolation_at_eighths!(u16, [0, 1, 255, 256, 32_767, 32_768, u16::MAX]);
        assert_interpolation_at_eighths!(
            i16,
            [
                i16::MIN,
                -16_385,
                -129,
                -128,
                -1,
                0,
                1,
                127,
                128,
                16_385,
                i16::MAX
            ]
        );
        assert_interpolation_at_eighths!(
            i32,
            [
                i32::MIN,
                i32::MIN + 1,
                -16_777_217,
                -16_777_216,
                -1,
                0,
                1,
                16_777_216,
                16_777_217,
                i32::MAX - 1,
                i32::MAX,
            ]
        );
    }

    fn normalized_input() -> impl Strategy<Value = UnitInterval> {
        prop_oneof![
            Just(0.0),
            Just(1.0),
            0.0_f32..=1.0,
            (0_u32..=1.0_f32.to_bits()).prop_map(f32::from_bits),
        ]
        .prop_map(unit_interval)
    }

    macro_rules! denormalization_properties {
        ($name:ident, $ty:ty) => {
            proptest! {
                #[test]
                fn $name(
                    first in any::<$ty>(),
                    second in any::<$ty>(),
                    left in normalized_input(),
                    right in normalized_input(),
                ) {
                    let min = first.min(second);
                    let max = first.max(second);
                    let range = CapabilityRange::<$ty>::new(min, max);
                    let lower = left.min(right);
                    let upper = left.max(right);
                    let lower_result = range.denormalize(lower);
                    let upper_result = range.denormalize(upper);
                    prop_assert!(range.contains(lower_result));
                    prop_assert!(range.contains(upper_result));
                    prop_assert!(lower_result <= upper_result);
                    prop_assert_eq!(range.denormalize(UnitInterval::MIN), min);
                    prop_assert_eq!(range.denormalize(UnitInterval::MAX), max);

                    let singleton = CapabilityRange::<$ty>::new(first, first);
                    prop_assert_eq!(singleton.denormalize(left), first);
                    prop_assert_eq!(singleton.denormalize(right), first);
                    prop_assert_eq!(singleton.denormalize(UnitInterval::MIN), first);
                    prop_assert_eq!(singleton.denormalize(UnitInterval::MAX), first);
                }
            }
        };
    }

    denormalization_properties!(denormalize_u8_stays_within_bounds_and_monotone, u8);
    denormalization_properties!(denormalize_u16_stays_within_bounds_and_monotone, u16);
    denormalization_properties!(denormalize_i8_stays_within_bounds_and_monotone, i8);
    denormalization_properties!(denormalize_i16_stays_within_bounds_and_monotone, i16);
    denormalization_properties!(denormalize_i32_stays_within_bounds_and_monotone, i32);

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
        let range = CapabilityRange::<u8>::new(1, 3);
        assert_eq!(range.validate("speed", 1).ok(), Some(1));
        assert_eq!(range.validate("speed", 3).ok(), Some(3));
        for value in [0, 4] {
            assert!(
                matches!(range.validate("speed", value), Err(Error::ParameterOutOfRange { parameter: "speed", value: actual, min: 1, max: 3, .. }) if actual == i32::from(value))
            );
        }
        let signed = CapabilityRange::<i16>::new(-20, 20);
        assert_eq!(signed.validate("pos", -20).ok(), Some(-20));
        assert!(matches!(
            signed.validate("pos", -21),
            Err(Error::ParameterOutOfRange {
                parameter: "pos",
                value: -21,
                min: -20,
                max: 20,
                ..
            })
        ));
        let wide = CapabilityRange::<i32>::new(-5, i32::MAX);
        assert_eq!(wide.validate("w", i32::MAX).ok(), Some(i32::MAX));
    }

    #[test]
    fn capability_domain_admits_bounds_and_refuses_gaps_through_one_rule() {
        let domain = CapabilityDomain::<u8>::with_gaps(0x00, 0x11, &[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(domain.gaps(), &[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(domain.bounds(), CapabilityRange::<u8>::new(0x00, 0x11));
        for admitted in [0x00, 0x05, 0x11] {
            assert!(domain.contains(admitted));
            assert_eq!(domain.validate("iris", admitted).ok(), Some(admitted));
        }
        for gap in 0x01..=0x04 {
            assert_eq!(domain.admission(gap), DomainAdmission::Gap);
            assert!(matches!(
                domain.validate("iris", gap),
                Err(Error::InvalidParameter { .. })
            ));
        }
        assert_eq!(domain.admission(0x12), DomainAdmission::OutOfBounds);
        assert!(matches!(
            domain.validate("iris", 0x12),
            Err(Error::ParameterOutOfRange {
                parameter: "iris",
                value: 18,
                min: 0,
                max: 17,
                ..
            })
        ));
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
