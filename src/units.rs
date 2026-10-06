//! Semantic unit types for VISCA protocol values.
//!
//! This module provides strongly-typed units for camera parameters,
//! enabling intuitive and type-safe API usage.

use std::{borrow::Cow, convert::TryFrom, fmt, str::FromStr};

use crate::{
    error::Error,
    types::{
        ColorTemp, ContrastLevel, FocusPosition, GainLevel, HueLevel, IrisLevel, PanSpeed,
        SaturationLevel, SharpnessLevel, TiltSpeed, ZoomPosition,
    },
};

/// Position in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Degrees<T = f32>(pub T);

/// Position in VISCA protocol units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViscaUnits<T>(pub T);

/// Checked unit interval value (`0.0..=1.0`).
///
/// This is the canonical public type for normalized camera positions such as
/// zoom position. It rejects values outside the unit interval as well as `NaN`
/// and infinities.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct UnitInterval(
    #[cfg_attr(feature = "schemars", validate(range(min = 0.0, max = 1.0)))] f32,
);

/// Percentage value (0.0 to 100.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Percentage<T = f32>(pub T);

/// Raw VISCA value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raw<T>(pub T);

/// Color temperature in Kelvin.
///
/// Converts to the wire value through [`ColorTemp::from_kelvin`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kelvin(pub u16);

/// Exposure time as a fraction of a second, such as `1/60`.
///
/// A `Fraction` is always stored in lowest terms with a nonzero denominator,
/// so equality is value equality: `1/60 == 2/120`. Its text form, used by
/// `Display`, `FromStr`, and serde, is `numerator/denominator`.
///
/// Shutter codes are camera-specific, so a fraction is converted to a code
/// only through a profile's shutter table:
/// [`Capabilities::shutter_speed_for`](crate::capabilities::Capabilities::shutter_speed_for).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "String", into = "String"))]
pub struct Fraction {
    numerator: u32,
    denominator: u32,
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for Fraction {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Fraction")
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^[0-9]+/[0-9]*[1-9][0-9]*$",
            "description": "Exposure time as `numerator/denominator` seconds, such as `1/60`.",
        })
    }
}

impl<T> Degrees<T> {
    /// Create a new position in degrees.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get the inner value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.0
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl From<f32> for Degrees<f32> {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<f64> for Degrees<f64> {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

impl From<f64> for Degrees<f32> {
    /// Creates a `Degrees<f32>` from an `f64` value.
    ///
    /// # Note on Precision
    /// This conversion performs a lossy cast from `f64` to `f32`.
    /// Precision may be lost during the conversion, but this is typically
    /// acceptable for camera positioning applications where the physical
    /// resolution of the motors is the limiting factor rather than
    /// floating-point precision.
    ///
    /// # Example
    /// ```
    /// use grafton_visca::units::Degrees;
    ///
    /// let degrees: Degrees = Degrees::from(45.123456789_f64);
    /// // The value is now stored as f32 internally
    /// ```
    fn from(value: f64) -> Self {
        Self(value as f32)
    }
}

impl<T> ViscaUnits<T> {
    /// Create a new position in VISCA units.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get the inner value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.0
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl UnitInterval {
    /// The lower bound of the unit interval.
    pub const ZERO: Self = Self(0.0);

    /// The upper bound of the unit interval.
    pub const ONE: Self = Self(1.0);

    /// Create a new unit interval value.
    ///
    /// # Errors
    /// Returns an error if `value` is outside `0.0..=1.0`, `NaN`, or infinite.
    pub fn new(value: f32) -> Result<Self, Error> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(Error::InvalidParameter {
                parameter: "unit_interval",
                value: Cow::Owned(value.to_string()),
                reason: Cow::Borrowed("Value must be finite and between 0.0 and 1.0"),
            });
        }
        Ok(Self(value))
    }

    /// Get the inner value.
    #[must_use]
    pub const fn value(self) -> f32 {
        self.0
    }

    /// Consume and return the inner value.
    #[must_use]
    pub const fn into_inner(self) -> f32 {
        self.0
    }

    /// Converts to percentage (0-100).
    #[must_use]
    pub fn to_percentage(self) -> u8 {
        (self.0 * 100.0).round() as u8
    }
}

impl TryFrom<f32> for UnitInterval {
    type Error = Error;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<UnitInterval> for f32 {
    fn from(value: UnitInterval) -> Self {
        value.0
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for UnitInterval {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_f32(self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for UnitInterval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <f32 as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl<T> Percentage<T> {
    /// Create a new percentage value.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get the inner value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.0
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

fn validate_percentage(value: f32, parameter: &'static str) -> Result<f32, Error> {
    if !value.is_finite() {
        return Err(Error::InvalidParameter {
            parameter,
            value: Cow::Owned(value.to_string()),
            reason: Cow::Borrowed("Value must be finite and between 0.0 and 100.0"),
        });
    }
    if !(0.0..=100.0).contains(&value) {
        return Err(Error::ParameterOutOfRange {
            parameter,
            value: value as i32,
            min: 0,
            max: 100,
        });
    }
    Ok(value)
}

impl<T> Raw<T> {
    /// Create a new raw value.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get the inner value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.0
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl Fraction {
    /// Creates a fraction in lowest terms, or `None` for a zero denominator.
    #[must_use]
    pub const fn new(numerator: u32, denominator: u32) -> Option<Self> {
        if denominator == 0 {
            return None;
        }
        let (mut a, mut b) = (numerator, denominator);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Some(Self {
            numerator: numerator / a,
            denominator: denominator / a,
        })
    }

    /// Returns the numerator in lowest terms.
    #[must_use]
    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    /// Returns the nonzero denominator in lowest terms.
    #[must_use]
    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    /// Get the decimal value of the fraction.
    #[must_use]
    pub fn as_decimal(&self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }
}

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}

impl FromStr for Fraction {
    type Err = Error;

    /// Parses `numerator/denominator`, such as `1/60`, into lowest terms. The
    /// denominator must be nonzero.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || Error::InvalidParameter {
            parameter: "fraction",
            value: Cow::Owned(text.to_owned()),
            reason: Cow::Borrowed("expected `numerator/denominator` with a nonzero denominator"),
        };
        let (numerator, denominator) = text.split_once('/').ok_or_else(invalid)?;
        let numerator = numerator.trim().parse().map_err(|_| invalid())?;
        let denominator = denominator.trim().parse().map_err(|_| invalid())?;
        Self::new(numerator, denominator).ok_or_else(invalid)
    }
}

impl TryFrom<String> for Fraction {
    type Error = Error;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<Fraction> for String {
    fn from(fraction: Fraction) -> Self {
        fraction.to_string()
    }
}

impl TryFrom<Raw<u16>> for ZoomPosition {
    type Error = Error;

    fn try_from(raw: Raw<u16>) -> Result<Self, Self::Error> {
        ZoomPosition::new(raw.0)
    }
}

impl From<Raw<u16>> for FocusPosition {
    fn from(raw: Raw<u16>) -> Self {
        FocusPosition::new(raw.0)
    }
}

impl TryFrom<Kelvin> for ColorTemp {
    type Error = Error;

    /// Converts through [`ColorTemp::from_kelvin`], the crate's single Kelvin
    /// mapping.
    fn try_from(kelvin: Kelvin) -> Result<Self, Self::Error> {
        ColorTemp::from_kelvin(kelvin.0)
    }
}

/// Implements the checked `Percentage` and `Raw` conversions for value types
/// whose domain starts at zero.
///
/// A percentage scales linearly onto `0..=MAX` and rounds to the nearest
/// value. A raw value is validated exactly like the type's `new`; neither
/// conversion clamps (#828: the former infallible `From<Raw<_>>` silently
/// mapped an out-of-range raw value to `MIN`).
macro_rules! scaled_value_conversions {
    ($($ty:ty: $raw:ty => $parameter:literal),* $(,)?) => {
        $(
            impl TryFrom<Percentage<f32>> for $ty {
                type Error = Error;

                fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
                    let percentage = validate_percentage(percentage.0, $parameter)?;
                    let value =
                        (percentage / 100.0 * f32::from(<$ty>::MAX.value())).round() as $raw;
                    <$ty>::new(value)
                }
            }

            impl TryFrom<Raw<$raw>> for $ty {
                type Error = Error;

                fn try_from(raw: Raw<$raw>) -> Result<Self, Self::Error> {
                    <$ty>::new(raw.0)
                }
            }
        )*
    };
}

scaled_value_conversions! {
    IrisLevel: u8 => "iris percentage",
    PanSpeed: u8 => "pan speed percentage",
    TiltSpeed: u8 => "tilt speed percentage",
    GainLevel: u8 => "gain percentage",
    SharpnessLevel: u8 => "sharpness percentage",
    ContrastLevel: u8 => "contrast percentage",
    SaturationLevel: u8 => "saturation percentage",
    HueLevel: u8 => "hue percentage",
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::types::FStop;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_raw_zoom_conversion_is_checked() {
        let zoom = ZoomPosition::try_from(Raw(0x4000_u16)).unwrap();
        assert_eq!(zoom.value(), 0x4000);

        assert!(ZoomPosition::try_from(Raw(0xFFFF_u16)).is_err());
    }

    #[test]
    fn test_fstop_iris_conversion() {
        let fstop = FStop::F2_8;
        let iris = IrisLevel::from(fstop);
        assert_eq!(iris.value(), 0x09);

        let fstop = FStop::Closed;
        let iris = IrisLevel::from(fstop);
        assert_eq!(iris.value(), 0x00);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn fraction_is_stored_in_lowest_terms_so_equality_is_value_equality() {
        let sixtieth: Fraction = "1/60".parse().unwrap();
        assert_eq!(Some(sixtieth), Fraction::new(1, 60));
        assert_eq!(sixtieth.to_string(), "1/60");
        assert_eq!(Fraction::new(2, 120), Some(sixtieth));
        assert_eq!("2/120".parse::<Fraction>().unwrap(), sixtieth);
        assert_ne!(Fraction::new(1, 50), Some(sixtieth));
        let zero = Fraction::new(0, 7).unwrap();
        assert_eq!((zero.numerator(), zero.denominator()), (0, 1));
        assert_eq!(Fraction::new(1, 0), None);
        for invalid in ["", "1", "1/0", "a/60", "1/60/2", "-1/60"] {
            assert!(invalid.parse::<Fraction>().is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn kelvin_conversion_is_color_temp_from_kelvin_and_round_trips_every_step() {
        for kelvin in (ColorTemp::MIN_KELVIN..=ColorTemp::MAX_KELVIN).step_by(100) {
            let via_try_from = ColorTemp::try_from(Kelvin(kelvin)).unwrap();
            assert_eq!(via_try_from, ColorTemp::from_kelvin(kelvin).unwrap());
            assert_eq!(via_try_from.to_kelvin(), kelvin, "{kelvin} K");
        }
        // 3200 K previously encoded as 0x0B (3600 K) through `TryFrom<Kelvin>`.
        assert_eq!(ColorTemp::try_from(Kelvin(3200)).unwrap().value(), 0x07);
        assert_eq!(ColorTemp::from_kelvin(3249).unwrap().to_kelvin(), 3200);
        assert_eq!(ColorTemp::from_kelvin(3250).unwrap().to_kelvin(), 3300);
        for out_of_range in [0, 2000, 2499, 8001, u16::MAX] {
            assert!(matches!(
                ColorTemp::try_from(Kelvin(out_of_range)),
                Err(Error::ParameterOutOfRange {
                    parameter: "kelvin",
                    min: 2500,
                    max: 8000,
                    ..
                })
            ));
        }
        assert_eq!(ColorTemp::MAX_KELVIN, 8000);
    }

    /// #828 (M5): `Raw(0xFF)` previously became pan speed 1 and `Raw(200)`
    /// gain 0 through an infallible `From`.
    #[test]
    fn raw_conversions_validate_instead_of_clamping() {
        assert_eq!(
            PanSpeed::try_from(Raw(0x18_u8)).ok().map(PanSpeed::value),
            Some(0x18)
        );
        assert!(PanSpeed::try_from(Raw(0x19_u8)).is_err());
        assert!(GainLevel::try_from(Raw(0x10_u8)).is_err());
        assert!(IrisLevel::try_from(Raw(0x1F_u8)).is_err());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn percentage_conversions_reject_invalid_values_and_reach_each_declared_maximum() {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 100.1] {
            assert!(
                IrisLevel::try_from(Percentage::new(invalid)).is_err(),
                "accepted {invalid}"
            );
        }

        let percentage = Percentage::new(100.0);
        let conversions = [
            (
                "iris",
                IrisLevel::try_from(percentage).map(|value| u16::from(value.value())),
                u16::from(IrisLevel::MAX.value()),
            ),
            (
                "pan speed",
                PanSpeed::try_from(percentage).map(|value| u16::from(value.value())),
                u16::from(PanSpeed::MAX.value()),
            ),
            (
                "tilt speed",
                TiltSpeed::try_from(percentage).map(|value| u16::from(value.value())),
                u16::from(TiltSpeed::MAX.value()),
            ),
            (
                "gain",
                GainLevel::try_from(percentage).map(|value| u16::from(value.value())),
                u16::from(GainLevel::MAX.value()),
            ),
            (
                "sharpness",
                SharpnessLevel::try_from(percentage).map(|value| u16::from(value.value())),
                u16::from(SharpnessLevel::MAX.value()),
            ),
            (
                "contrast",
                ContrastLevel::try_from(percentage).map(|value| u16::from(value.value())),
                u16::from(ContrastLevel::MAX.value()),
            ),
            (
                "saturation",
                SaturationLevel::try_from(percentage).map(|value| u16::from(value.value())),
                u16::from(SaturationLevel::MAX.value()),
            ),
            (
                "hue",
                HueLevel::try_from(percentage).map(|value| u16::from(value.value())),
                u16::from(HueLevel::MAX.value()),
            ),
        ];

        for (name, actual, maximum) in conversions {
            assert_eq!(actual.unwrap(), maximum, "{name}");
        }
    }
}
