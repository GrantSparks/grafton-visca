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

const RANGE_BOUNDS: &str = "capability range minimum exceeds maximum";
const DOMAIN_BOUNDS: &str = "capability domain bounds must be ordered and its gaps sorted, unique, and strictly inside them";

impl<T: PartialOrd> CapabilityRange<T> {
    /// Creates an inclusive capability range from checked runtime bounds.
    ///
    /// # Errors
    /// Returns a profile-field error when the bounds are unordered or inverted.
    ///
    /// ```
    /// use grafton_visca::capabilities::CapabilityRange;
    /// let range = CapabilityRange::<i32>::new(-100, 100)?;
    /// assert!(range.contains(0));
    /// # Ok::<(), grafton_visca::Error>(())
    /// ```
    pub fn new(min: T, max: T) -> crate::Result<Self> {
        if min <= max {
            Ok(Self { min, max })
        } else {
            Err(crate::profiles::invalid_profile_fields(
                &["capability_range"],
                RANGE_BOUNDS,
            ))
        }
    }
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
        Self::new(wire.min, wire.max).map_err(serde::de::Error::custom)
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
            /// Creates a checked domain whose source table does not list `gaps`.
            ///
            /// Pass an empty slice or vector for a domain without gaps. Runtime
            /// gap vectors are owned by the resulting domain.
            ///
            /// # Errors
            /// Returns a profile-field error when bounds are inverted or gaps
            /// are not sorted, unique and strictly inside the bounds.
            ///
            /// ```
            /// use grafton_visca::capabilities::CapabilityDomain;
            /// let domain = CapabilityDomain::<u16>::new(0, 9, vec![2, 3])?;
            /// assert!(!domain.contains(2));
            /// # Ok::<(), grafton_visca::Error>(())
            /// ```
            pub fn new(
                min: $ty,
                max: $ty,
                gaps: impl Into<Cow<'static, [$ty]>>,
            ) -> crate::Result<Self> {
                let gaps = gaps.into();
                if !Self::is_well_formed(min, max, &gaps) {
                    return Err(crate::profiles::invalid_profile_fields(
                        &["capability_domain"],
                        DOMAIN_BOUNDS,
                    ));
                }
                Ok(Self { min, max, gaps })
            }

            /// Checked registry literals are evaluated in a const block.
            pub(crate) const fn from_registry(min: $ty, max: $ty, gaps: &'static [$ty]) -> Self {
                assert!(
                    Self::is_well_formed(min, max, gaps),
                    "invalid registry capability domain"
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
                    DomainAdmission::OutOfBounds => self.bounds().validate(parameter, value),
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
                Self::new(wire.min, wire.max, wire.gaps).map_err(serde::de::Error::custom)
            }
        }
    };
}

impl_capability_domain!(u8);
impl_capability_domain!(u16);

const F32_FRACTION_BITS: u32 = f32::MANTISSA_DIGITS - 1;
const F32_IMPLICIT_BIT: u32 = 1 << F32_FRACTION_BITS;
const F32_EXPONENT_BIAS: u32 = 127;

/// Rounds the ratio of an interior offset and its span (both at most 32 bits).
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn normalize_ratio(offset: u32, span: u32) -> f32 {
    if span <= 1 << f32::MANTISSA_DIGITS {
        // Both operands are exact in binary32.
        return offset as f32 / span as f32;
    }

    // Shift the numerator until the ratio is in [1, 2), then compute its
    // binary32 significand. The scaled numerator uses at most 56 bits.
    let mut shift = span.ilog2() - offset.ilog2();
    let mut numerator = u64::from(offset) << shift;
    let span = u64::from(span);
    if numerator < span {
        numerator <<= 1;
        shift += 1;
    }
    let scaled = numerator << F32_FRACTION_BITS;
    let significand = scaled / span;
    let doubled_remainder = 2 * (scaled % span);
    let round_up = doubled_remainder > span || (doubled_remainder == span && significand & 1 != 0);
    let fraction = (significand + u64::from(round_up) - u64::from(F32_IMPLICIT_BIT)) as u32;
    // A rounded significand of 2 carries into the exponent.
    f32::from_bits(((F32_EXPONENT_BIAS - shift) << F32_FRACTION_BITS) + fraction)
}

/// Interpolates using the exact binary32 fraction and a span of at most 32 bits.
fn interpolate_integer(min: i64, span: i64, value: f32) -> i64 {
    let bits = value.to_bits();
    let exponent = bits >> F32_FRACTION_BITS;
    let fractional_bits = F32_EXPONENT_BIAS + F32_FRACTION_BITS - exponent;
    if fractional_bits > i32::BITS + f32::MANTISSA_DIGITS {
        // Even the largest possible product is less than half a step. This
        // also covers subnormals, so the remaining inputs have an implicit bit.
        return min;
    }

    let significand = (bits & (F32_IMPLICIT_BIT - 1)) | F32_IMPLICIT_BIT;
    let offset = span * i64::from(significand);
    let floor = min + (offset >> fractional_bits);
    let half = 1_i64 << (fractional_bits - 1);
    let remainder = offset & ((half << 1) - 1);
    // At a negative tie the floor is already the integer away from zero.
    floor + i64::from(remainder > half || (remainder == half && floor >= 0))
}

macro_rules! impl_capability_range {
    ($ty:ty) => {
        impl CapabilityRange<$ty> {
            /// Normalizes a supported value within this profile's bounds.
            ///
            /// The result is the nearest representable normalized value, with
            /// ties rounded to even. A single-value domain normalizes to zero.
            /// Values outside the declared bounds are rejected before conversion.
            pub fn normalize(self, value: $ty) -> crate::Result<crate::types::UnitInterval> {
                self.validate("capability value", value)?;
                if value == self.min {
                    return Ok(crate::types::UnitInterval::MIN);
                }
                if value == self.max {
                    return Ok(crate::types::UnitInterval::MAX);
                }
                let min = i32::from(self.min);
                let offset = i32::from(value).abs_diff(min);
                let span = i32::from(self.max).abs_diff(min);
                crate::types::UnitInterval::new(normalize_ratio(offset, span))
            }

            /// Maps a normalized value into this profile's inclusive domain.
            ///
            /// A single-value domain always returns its sole value; normalized
            /// zero and one return the exact endpoints. Interpolation rounds
            /// to the nearest integer with exact halves away from zero. The
            /// result stays within the declared bounds.
            #[must_use]
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            pub fn denormalize(self, value: crate::types::UnitInterval) -> $ty {
                if self.min == self.max || value == crate::types::UnitInterval::MIN {
                    return self.min;
                }
                if value == crate::types::UnitInterval::MAX {
                    return self.max;
                }
                let min = i64::from(self.min);
                let span = i64::from(self.max) - min;
                interpolate_integer(min, span, value.value()) as $ty
            }

            /// Checked registry literals are evaluated in a const block.
            #[allow(dead_code)] // Not every supported scalar width occurs in the registry.
            pub(crate) const fn from_registry(min: $ty, max: $ty) -> Self {
                assert!(min <= max, "invalid registry capability range");
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
#[allow(clippy::expect_used)]
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
            let range = CapabilityRange::<i32>::new(min, max).expect("valid capability bounds");
            let actual = range.denormalize(unit_interval(normalized));
            assert!(
                range.contains(actual),
                "returned {actual} outside {range:?}"
            );
            assert_eq!(actual, expected, "{range:?} at {normalized}");
        }
    }

    #[test]
    fn denormalize_uses_the_actual_side_of_a_half_step() {
        for (min, max, bits, expected) in [
            (i32::MAX - 1, i32::MAX, 0x3eff_ffff, i32::MAX - 1),
            (-i32::MAX, -i32::MAX + 1, 0x3f00_0001, -i32::MAX + 1),
            (0, i32::MAX, 0x3f00_0001, 1_073_741_951),
        ] {
            let range = CapabilityRange::<i32>::new(min, max).expect("valid capability bounds");
            let value = unit_interval(f32::from_bits(bits));
            assert_eq!(rational_interpolation(min, max, value), expected);
            assert_eq!(range.denormalize(value), expected);
        }
        let range = CapabilityRange::<u16>::new(8_198, u16::MAX).expect("valid capability bounds");
        let value = unit_interval(f32::from_bits(0x3712_4db7));
        assert_eq!(
            rational_interpolation(8_198, i32::from(u16::MAX), value),
            8_198
        );
        assert_eq!(range.denormalize(value), 8_198);
    }

    #[test]
    fn normalize_rounds_the_exact_ratio() -> crate::Result<()> {
        for (max, value, bits) in [
            (16_777_217, 16_777_216, 0x3f7f_ffff),
            (1_090_519_039, 545_259_552, 0x3f00_0001), // Just above an even midpoint.
            (33_554_432, 16_777_217, 0x3f00_0000),     // Exact midpoint, lower wins.
            (33_554_432, 16_777_219, 0x3f00_0002),     // Exact midpoint, upper wins.
        ] {
            let range = CapabilityRange::<i32>::new(0, max)?;
            let normalized = range.normalize(value)?;
            assert_eq!(normalized.value().to_bits(), bits);
            assert_nearest_ratio(0, max, value, normalized);
        }
        let range = CapabilityRange::<i32>::new(0, 16_777_217)?;
        assert_eq!(range.denormalize(range.normalize(16_777_216)?), 16_777_216);
        Ok(())
    }

    // Use a signed weighted sum of the endpoints to obtain the interpolated
    // coordinate as an exact rational before applying ties-away rounding.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn rational_interpolation(min: i32, max: i32, value: UnitInterval) -> i32 {
        const DENOMINATOR: i128 = 1 << 64;
        let value = f64::from(value.value());
        if value < 2.0_f64.powi(-33) {
            return min; // The integral range is narrower than 2^32.
        }
        // At 2^-33 and above, scaling by 2^64 is exact and integral.
        let weight = (value * DENOMINATOR as f64) as i128;
        let numerator = i128::from(min) * (DENOMINATOR - weight) + i128::from(max) * weight;
        let rounded = (numerator.abs() + DENOMINATOR / 2) / DENOMINATOR;
        i32::try_from(numerator.signum() * rounded).expect("interpolation stays in the range")
    }

    // Check both boundaries of the returned float's rounding interval by
    // cross-multiplying the exact midpoints and integer ratio.
    #[allow(clippy::cast_possible_truncation)]
    fn assert_nearest_ratio(min: i32, max: i32, value: i32, normalized: UnitInterval) {
        if value == min {
            assert_eq!(normalized, UnitInterval::MIN);
            return;
        }
        if value == max {
            assert_eq!(normalized, UnitInterval::MAX);
            return;
        }

        let actual = normalized.value();
        assert!(actual >= 2.0_f32.powi(-32));
        let span = i128::from(max) - i128::from(min);
        let offset = i128::from(value) - i128::from(min);
        let ratio_scaled = offset << 57;
        // Binary32 midpoints at and above the smallest attainable ratio
        // (2^-32) are exact multiples of 2^-57.
        let midpoint_scaled = |neighbor| {
            let sum = f64::from(actual) + f64::from(neighbor);
            (sum * 2.0_f64.powi(56)) as i128
        };
        let lower = span * midpoint_scaled(actual.next_down());
        let upper = span * midpoint_scaled(actual.next_up());
        let even = actual.to_bits() & 1 == 0;
        assert!(lower < ratio_scaled || (lower == ratio_scaled && even));
        assert!(ratio_scaled < upper || (ratio_scaled == upper && even));
    }

    fn i32_range_endpoints() -> impl Strategy<Value = (i32, i32)> {
        prop_oneof![
            (any::<i32>(), any::<i32>()).prop_map(|(a, b)| (a.min(b), a.max(b))),
            (i32::MIN..=0, 0..=i32::MAX),
            any::<i32>().prop_map(|max| (i32::MIN, max)),
            any::<i32>().prop_map(|min| (min, i32::MAX)),
            any::<i32>().prop_map(|endpoint| (endpoint, endpoint)),
        ]
    }

    proptest! {
        #[test]
        fn i32_normalization_and_interpolation_match_rational_oracles(
            (min, max) in i32_range_endpoints(),
            index in any::<u32>(),
            other_index in any::<u32>(),
            value in normalized_input(),
        ) {
            let range = CapabilityRange::<i32>::new(min, max).expect("valid capability bounds");
            let span = u32::try_from(i64::from(max) - i64::from(min)).expect("span fits in u32");
            let coordinate = |index| {
                let offset = i64::from(index) % (i64::from(span) + 1);
                i32::try_from(i64::from(min) + offset).expect("coordinate stays in range")
            };
            let first = coordinate(index);
            let second = coordinate(other_index);
            let normalized_first = range.normalize(first).expect("coordinate stays in range");
            let normalized_second = range.normalize(second).expect("coordinate stays in range");
            assert_nearest_ratio(min, max, first, normalized_first);
            assert_nearest_ratio(min, max, second, normalized_second);
            if first <= second {
                prop_assert!(normalized_first <= normalized_second);
            } else {
                prop_assert!(normalized_second <= normalized_first);
            }
            assert_nearest_ratio(min, max, min, range.normalize(min).expect("minimum is included"));
            assert_nearest_ratio(min, max, max, range.normalize(max).expect("maximum is included"));
            prop_assert_eq!(range.denormalize(value), rational_interpolation(min, max, value));

            if span > 0 {
                // Pick a physical half step and probe the nearest binary32
                // input as well as one representable value on either side.
                #[allow(clippy::cast_possible_truncation)]
                let half_step = ((f64::from(index % span) + 0.5) / f64::from(span)) as f32;
                for neighbor in [half_step.next_down(), half_step, half_step.next_up()] {
                    if let Ok(neighbor) = UnitInterval::new(neighbor) {
                        prop_assert_eq!(
                            range.denormalize(neighbor),
                            rational_interpolation(min, max, neighbor)
                        );
                    }
                }
            }
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
            let range = CapabilityRange::<i32>::new(min, max).expect("valid capability bounds");
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
                    let range =
                        CapabilityRange::<$ty>::new(min, max).expect("valid capability bounds");
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
                    let range = CapabilityRange::<$ty>::new(min, max).expect("valid capability bounds");
                    let lower = left.min(right);
                    let upper = left.max(right);
                    let lower_result = range.denormalize(lower);
                    let upper_result = range.denormalize(upper);
                    prop_assert!(range.contains(lower_result));
                    prop_assert!(range.contains(upper_result));
                    prop_assert!(lower_result <= upper_result);
                    prop_assert_eq!(range.denormalize(UnitInterval::MIN), min);
                    prop_assert_eq!(range.denormalize(UnitInterval::MAX), max);

                    let singleton = CapabilityRange::<$ty>::new(first, first).expect("valid capability bounds");
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
        assert_eq!(
            CapabilityRange::<u8>::new(1_u8, 3)
                .expect("valid capability bounds")
                .as_inclusive(),
            1..=3
        );
        assert_eq!(
            CapabilityRange::<u16>::new(0_u16, 10)
                .expect("valid capability bounds")
                .as_inclusive(),
            0..=10
        );
        assert_eq!(
            CapabilityRange::<i8>::new(-7_i8, 7)
                .expect("valid capability bounds")
                .as_inclusive(),
            -7..=7
        );
        assert_eq!(
            CapabilityRange::<i16>::new(-170_i16, 170)
                .expect("valid capability bounds")
                .as_inclusive(),
            -170..=170
        );
    }

    #[test]
    fn validate_accepts_bounds_and_rejects_outside_without_clamping() {
        let range = CapabilityRange::<u8>::new(1, 3).expect("valid capability bounds");
        assert_eq!(range.validate("speed", 1).ok(), Some(1));
        assert_eq!(range.validate("speed", 3).ok(), Some(3));
        for value in [0, 4] {
            assert!(
                matches!(range.validate("speed", value), Err(Error::ParameterOutOfRange { parameter: "speed", value: actual, min: 1, max: 3, .. }) if actual == i32::from(value))
            );
        }
        let signed = CapabilityRange::<i16>::new(-20, 20).expect("valid capability bounds");
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
        let wide = CapabilityRange::<i32>::new(-5, i32::MAX).expect("valid capability bounds");
        assert_eq!(wide.validate("w", i32::MAX).ok(), Some(i32::MAX));
    }

    #[test]
    fn capability_domain_admits_bounds_and_refuses_gaps_through_one_rule() {
        let domain = CapabilityDomain::<u8>::new(0x00, 0x11, &[0x01, 0x02, 0x03, 0x04])
            .expect("valid capability bounds");
        assert_eq!(domain.gaps(), &[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(
            domain.bounds(),
            CapabilityRange::<u8>::new(0x00, 0x11).expect("valid capability bounds")
        );
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
                CapabilityDomain::<u16>::new(min, max, gaps).is_err(),
                "{min}..={max} {gaps:?}"
            );
        }
    }

    #[test]
    fn capability_range_rejects_inverted_ranges() {
        assert!(CapabilityRange::<u8>::new(2_u8, 1).is_err());
        assert!(CapabilityRange::<u16>::new(2_u16, 1).is_err());
        assert!(CapabilityRange::<i8>::new(2_i8, 1).is_err());
        assert!(CapabilityRange::<i16>::new(2_i16, 1).is_err());
        assert!(CapabilityRange::<i32>::new(2_i32, 1).is_err());
    }

    #[test]
    fn capability_domain_owns_runtime_gaps() {
        let gaps = vec![2_u16, 3, 7];
        let domain = CapabilityDomain::<u16>::new(1, 9, gaps).expect("runtime gaps");
        assert_eq!(domain.gaps(), &[2, 3, 7]);
        assert!(domain.contains(1));
        assert!(domain.contains(9));
        assert!(!domain.contains(7));
        assert!(CapabilityDomain::<u16>::new(1, 9, vec![9]).is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    #[allow(clippy::panic)]
    fn capability_constructor_and_serde_validate_identical_invariants() {
        for (min, max, gaps) in [
            (0_u16, 9, vec![]),
            (1, 9, vec![2, 7]),
            (1, 9, vec![9]),
            (1, 9, vec![0]),
            (1, 9, vec![7, 2]),
            (1, 9, vec![2, 2]),
            (9, 1, vec![]),
        ] {
            let domain = CapabilityDomain::<u16>::new(min, max, gaps.clone());
            let decoded = serde_json::from_value::<CapabilityDomain<u16>>(
                serde_json::json!({"min": min, "max": max, "gaps": gaps}),
            );
            match (domain, decoded) {
                (Ok(domain), Ok(decoded)) => assert_eq!(domain, decoded),
                (Err(error), Err(decoded)) => assert_eq!(error.to_string(), decoded.to_string()),
                pair => panic!("constructor and serde disagree: {pair:?}"),
            }
            let range = CapabilityRange::<u16>::new(min, max);
            let decoded = serde_json::from_value::<CapabilityRange<u16>>(
                serde_json::json!({"min": min, "max": max}),
            );
            match (range, decoded) {
                (Ok(range), Ok(decoded)) => assert_eq!(range, decoded),
                (Err(error), Err(decoded)) => assert_eq!(error.to_string(), decoded.to_string()),
                pair => panic!("constructor and serde disagree: {pair:?}"),
            }
        }
    }

    #[test]
    fn hand_built_zoom_with_inverted_digital_limits_has_no_digital_range() {
        let zoom = crate::capabilities::ZoomCaps {
            optical_max: 0x4000,
            digital_max: Some(0x1000),
            speed: CapabilityRange::<u8>::new(0, 7).expect("speed"),
            direct: true,
            variable: true,
            optical_ratio: None,
        };
        assert_eq!(zoom.digital_range(), None);
        assert_eq!(zoom.optical_range().as_inclusive(), 0..=0x4000);
    }

    #[test]
    fn capability_range_allows_single_value_ranges() {
        let range = CapabilityRange::<u8>::new(5_u8, 5).expect("valid capability bounds");

        assert_eq!(range.min(), 5);
        assert_eq!(range.max(), 5);
        assert!(range.contains(5));
        assert!(!range.contains(4));
        assert_eq!(range.as_inclusive(), 5..=5);
    }

    #[test]
    fn capability_range_represents_full_width_u8_ranges() {
        let range = CapabilityRange::<u8>::new(0_u8, u8::MAX).expect("valid capability bounds");

        assert!(range.contains(0));
        assert!(range.contains(u8::MAX));
        assert_eq!(range.as_inclusive(), 0..=u8::MAX);
    }

    #[test]
    fn capability_range_methods_work_for_supported_primitives() {
        let u8_range = CapabilityRange::<u8>::new(1_u8, 3).expect("valid capability bounds");
        assert_eq!(u8_range.min(), 1);
        assert_eq!(u8_range.max(), 3);
        assert!(u8_range.contains(2));
        assert_eq!(u8_range.as_inclusive(), 1..=3);

        let u16_range = CapabilityRange::<u16>::new(10_u16, 12).expect("valid capability bounds");
        assert_eq!(u16_range.min(), 10);
        assert_eq!(u16_range.max(), 12);
        assert!(u16_range.contains(11));
        assert_eq!(u16_range.as_inclusive(), 10..=12);

        let i8_range = CapabilityRange::<i8>::new(-2_i8, 2).expect("valid capability bounds");
        assert_eq!(i8_range.min(), -2);
        assert_eq!(i8_range.max(), 2);
        assert!(i8_range.contains(0));
        assert_eq!(i8_range.as_inclusive(), -2..=2);

        let i16_range = CapabilityRange::<i16>::new(-20_i16, 20).expect("valid capability bounds");
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
