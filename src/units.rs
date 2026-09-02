//! Semantic unit types for VISCA protocol values.
//!
//! This module provides strongly-typed units for camera parameters,
//! enabling intuitive and type-safe API usage.

use std::{borrow::Cow, convert::TryFrom};

use crate::{
    error::Error,
    types::{ColorTemp, FocusPosition, IrisLevel, PanSpeed, ShutterSpeed, TiltSpeed, ZoomPosition},
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

/// Magnification factor (e.g., 1.0x, 10.0x).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Magnification<T = f32>(pub T);

/// Color temperature in Kelvin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kelvin(pub u16);

/// Shutter speed as a fraction (numerator, denominator).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction {
    /// Numerator of the fraction (e.g., 1 for 1/60).
    pub numerator: u32,
    /// Denominator of the fraction (e.g., 60 for 1/60).
    pub denominator: u32,
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

impl<T> Magnification<T> {
    /// Create a new magnification value.
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
    /// Create a new fraction.
    #[must_use]
    pub fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// Get the decimal value of the fraction.
    #[must_use]
    pub fn as_decimal(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
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

impl TryFrom<Percentage<f32>> for IrisLevel {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "iris percentage",
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 0x10 as f32) as u8;
        IrisLevel::new(value)
    }
}

impl TryFrom<Fraction> for ShutterSpeed {
    type Error = Error;

    fn try_from(fraction: Fraction) -> Result<Self, Self::Error> {
        let value = match (fraction.numerator, fraction.denominator) {
            (1, 60) => 0x07,    // Actual 1/60
            (1, 100) => 0x08,   // Actual 1/100
            (1, 125) => 0x09,   // Close to 1/120
            (1, 250) => 0x0A,   // Actual 1/250
            (1, 500) => 0x0B,   // Actual 1/500
            (1, 1000) => 0x0C,  // Actual 1/1000
            (1, 2000) => 0x0D,  // Actual 1/2000
            (1, 4000) => 0x0E,  // Actual 1/4000
            (1, 10000) => 0x11, // Actual 1/10000
            _ => {
                return Err(Error::InvalidParameter {
                    parameter: "shutter_speed",
                    value: {
                        let num = fraction.numerator;
                        let den = fraction.denominator;
                        Cow::Owned(format!("{num}/{den}"))
                    },
                    reason: Cow::Borrowed("Unsupported shutter speed value"),
                })
            }
        };
        ShutterSpeed::new(value as u16)
    }
}

impl TryFrom<Kelvin> for ColorTemp {
    type Error = Error;

    fn try_from(kelvin: Kelvin) -> Result<Self, Self::Error> {
        if kelvin.0 < 2000 || kelvin.0 > 8000 {
            return Err(Error::ParameterOutOfRange {
                parameter: "color temperature",
                value: kelvin.0 as i32,
                min: 2000,
                max: 8000,
            });
        }
        let normalized = (kelvin.0 - 2000) as f32 / 6000.0;
        let value = (normalized * 0x37 as f32) as u16;
        ColorTemp::new(value)
    }
}

impl TryFrom<Percentage<f32>> for PanSpeed {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan speed percentage",
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 24.0) as u8;
        PanSpeed::new(value)
    }
}

impl TryFrom<Percentage<f32>> for TiltSpeed {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt speed percentage",
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 24.0) as u8;
        TiltSpeed::new(value)
    }
}

impl From<Raw<u8>> for PanSpeed {
    fn from(raw: Raw<u8>) -> Self {
        PanSpeed::new(raw.0).unwrap_or(PanSpeed::MIN)
    }
}

impl From<Raw<u8>> for TiltSpeed {
    fn from(raw: Raw<u8>) -> Self {
        TiltSpeed::new(raw.0).unwrap_or(TiltSpeed::MIN)
    }
}

impl TryFrom<Percentage<f32>> for crate::types::GainLevel {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "gain percentage",
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 7.0).round() as u8;
        crate::types::GainLevel::new(value)
    }
}

impl From<Raw<u8>> for crate::types::GainLevel {
    fn from(raw: Raw<u8>) -> Self {
        crate::types::GainLevel::new(raw.0).unwrap_or(crate::types::GainLevel::MIN)
    }
}

impl TryFrom<Percentage<f32>> for crate::types::SharpnessLevel {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "sharpness percentage",
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 7.0).round() as u8;
        crate::types::SharpnessLevel::new(value)
    }
}

impl From<Raw<u8>> for crate::types::SharpnessLevel {
    fn from(raw: Raw<u8>) -> Self {
        crate::types::SharpnessLevel::new(raw.0).unwrap_or(crate::types::SharpnessLevel::MIN)
    }
}

impl TryFrom<Percentage<f32>> for crate::types::BrightnessLevel {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "brightness percentage",
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 0x11 as f32).round() as u16;
        crate::types::BrightnessLevel::new(value)
    }
}

impl From<Raw<u16>> for crate::types::BrightnessLevel {
    fn from(raw: Raw<u16>) -> Self {
        crate::types::BrightnessLevel::new(raw.0).unwrap_or(crate::types::BrightnessLevel::MIN)
    }
}

impl TryFrom<Percentage<f32>> for crate::types::ContrastLevel {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "contrast percentage",
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 14.0).round() as u8;
        crate::types::ContrastLevel::new(value)
    }
}

impl From<Raw<u8>> for crate::types::ContrastLevel {
    fn from(raw: Raw<u8>) -> Self {
        crate::types::ContrastLevel::new(raw.0).unwrap_or(crate::types::ContrastLevel::MIN)
    }
}

impl TryFrom<Percentage<f32>> for crate::types::SaturationLevel {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "saturation percentage",
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 0x0E as f32).round() as u8;
        crate::types::SaturationLevel::new(value)
    }
}

impl From<Raw<u8>> for crate::types::SaturationLevel {
    fn from(raw: Raw<u8>) -> Self {
        crate::types::SaturationLevel::new(raw.0).unwrap_or(crate::types::SaturationLevel::MIN)
    }
}

impl TryFrom<Percentage<f32>> for crate::types::HueLevel {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "hue percentage",
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 0x0E as f32).round() as u8;
        crate::types::HueLevel::new(value)
    }
}

impl From<Raw<u8>> for crate::types::HueLevel {
    fn from(raw: Raw<u8>) -> Self {
        crate::types::HueLevel::new(raw.0).unwrap_or(crate::types::HueLevel::MIN)
    }
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
    fn full_tilt_speed_percentage_reaches_the_public_maximum() {
        let speed = TiltSpeed::try_from(Percentage(100.0)).unwrap();
        assert_eq!(speed.value(), 24);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_shutter_fraction_conversion() {
        let fraction = Fraction::new(1, 60);
        let shutter = ShutterSpeed::try_from(fraction).unwrap();
        assert_eq!(shutter.value(), 0x07);

        let fraction = Fraction::new(1, 1000);
        let shutter = ShutterSpeed::try_from(fraction).unwrap();
        assert_eq!(shutter.value(), 0x0C);
    }
}
