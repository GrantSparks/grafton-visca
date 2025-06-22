//! Semantic unit types for VISCA protocol values.
//!
//! This module provides strongly-typed units for camera parameters,
//! enabling intuitive and type-safe API usage.

use crate::error::Error;
use crate::types::{
    ColorTemperature, FocusPosition, IrisLevel, PanSpeed, ShutterSpeed, TiltSpeed, ZoomPosition,
};
use std::convert::TryFrom;

/// Position in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Degrees<T = f32>(pub T);

/// Position in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radians<T = f32>(pub T);

/// Position in VISCA protocol units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViscaUnits<T>(pub T);

/// Normalized position (0.0 to 1.0 or -1.0 to 1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Normalized<T = f32>(pub T);

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
    pub fn value(&self) -> &T {
        &self.0
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Radians<T> {
    /// Create a new position in radians.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get the inner value.
    pub fn value(&self) -> &T {
        &self.0
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> ViscaUnits<T> {
    /// Create a new position in VISCA units.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get the inner value.
    pub fn value(&self) -> &T {
        &self.0
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Normalized<T> {
    /// Create a new normalized position.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get the inner value.
    pub fn value(&self) -> &T {
        &self.0
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Percentage<T> {
    /// Create a new percentage value.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get the inner value.
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
    pub fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// Get the decimal value of the fraction.
    pub fn as_decimal(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

// Conversion implementations for degrees/radians
impl From<Degrees<f32>> for Radians<f32> {
    fn from(degrees: Degrees<f32>) -> Self {
        Radians(degrees.0.to_radians())
    }
}

impl From<Radians<f32>> for Degrees<f32> {
    fn from(radians: Radians<f32>) -> Self {
        Degrees(radians.0.to_degrees())
    }
}

// Conversion implementations for ZoomPosition
impl TryFrom<Percentage<f32>> for ZoomPosition {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "zoom percentage".to_string(),
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        // Convert percentage to VISCA units (0x0000 - 0x7000)
        let value = (percentage.0 / 100.0 * 0x7000 as f32) as u16;
        ZoomPosition::new(value)
    }
}

impl TryFrom<Normalized<f32>> for ZoomPosition {
    type Error = Error;

    fn try_from(normalized: Normalized<f32>) -> Result<Self, Self::Error> {
        if normalized.0 < 0.0 || normalized.0 > 1.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "zoom normalized".to_string(),
                value: (normalized.0 * 100.0) as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (normalized.0 * 0x7000 as f32) as u16;
        ZoomPosition::new(value)
    }
}

impl TryFrom<Magnification<f32>> for ZoomPosition {
    type Error = Error;

    fn try_from(magnification: Magnification<f32>) -> Result<Self, Self::Error> {
        // Assuming 1.0x = 0x0000, 30.0x = 0x7000 for a 30x camera
        // This would need to be adjusted based on camera profile
        if magnification.0 < 1.0 || magnification.0 > 30.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "zoom magnification".to_string(),
                value: magnification.0 as i32,
                min: 1,
                max: 30,
            });
        }
        let normalized = (magnification.0 - 1.0) / 29.0;
        let value = (normalized * 0x7000 as f32) as u16;
        ZoomPosition::new(value)
    }
}

impl From<Raw<u16>> for ZoomPosition {
    fn from(raw: Raw<u16>) -> Self {
        // Trust the user knows what they're doing with raw values
        ZoomPosition::new(raw.0).unwrap_or(ZoomPosition::MIN)
    }
}

// Conversion implementations for FocusPosition
impl TryFrom<Percentage<f32>> for FocusPosition {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "focus percentage".to_string(),
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        // Convert percentage to VISCA units (0x1000 - 0xF000)
        let range = 0xF000 - 0x1000;
        let value = 0x1000 + (percentage.0 / 100.0 * range as f32) as u16;
        FocusPosition::new(value)
    }
}

impl TryFrom<Normalized<f32>> for FocusPosition {
    type Error = Error;

    fn try_from(normalized: Normalized<f32>) -> Result<Self, Self::Error> {
        if normalized.0 < 0.0 || normalized.0 > 1.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "focus normalized".to_string(),
                value: (normalized.0 * 100.0) as i32,
                min: 0,
                max: 100,
            });
        }
        let range = 0xF000 - 0x1000;
        let value = 0x1000 + (normalized.0 * range as f32) as u16;
        FocusPosition::new(value)
    }
}

impl From<Raw<u16>> for FocusPosition {
    fn from(raw: Raw<u16>) -> Self {
        FocusPosition::new(raw.0).unwrap_or(FocusPosition::MIN)
    }
}

// Note: FStop to IrisLevel conversion is already implemented in types.rs

impl TryFrom<Percentage<f32>> for IrisLevel {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "iris percentage".to_string(),
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        // Convert percentage to iris level (0x00 - 0x10)
        let value = (percentage.0 / 100.0 * 0x10 as f32) as u8;
        IrisLevel::new(value)
    }
}

// Conversion implementations for ShutterSpeed
impl TryFrom<Fraction> for ShutterSpeed {
    type Error = Error;

    fn try_from(fraction: Fraction) -> Result<Self, Self::Error> {
        // Convert common shutter speeds to VISCA values
        // Based on the G2_VALID_VALUES from types.rs
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
                return Err(Error::InvalidParameter(format!(
                    "Unsupported shutter speed: {}/{}",
                    fraction.numerator, fraction.denominator
                )))
            }
        };
        ShutterSpeed::new(value as u16)
    }
}

// Conversion implementations for ColorTemperature
impl TryFrom<Kelvin> for ColorTemperature {
    type Error = Error;

    fn try_from(kelvin: Kelvin) -> Result<Self, Self::Error> {
        // PTZOptics G2 supports 2000K to 8000K
        if kelvin.0 < 2000 || kelvin.0 > 8000 {
            return Err(Error::ParameterOutOfRange {
                parameter: "color temperature".to_string(),
                value: kelvin.0 as i32,
                min: 2000,
                max: 8000,
            });
        }
        // Map to VISCA units (needs camera-specific mapping)
        // This is a simplified linear mapping
        let normalized = (kelvin.0 - 2000) as f32 / 6000.0;
        let value = (normalized * 0x37 as f32) as u16;
        ColorTemperature::new(value)
    }
}

// Conversion implementations for Pan/Tilt speeds
impl TryFrom<Percentage<f32>> for PanSpeed {
    type Error = Error;

    fn try_from(percentage: Percentage<f32>) -> Result<Self, Self::Error> {
        if percentage.0 < 0.0 || percentage.0 > 100.0 {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan speed percentage".to_string(),
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
                parameter: "tilt speed percentage".to_string(),
                value: percentage.0 as i32,
                min: 0,
                max: 100,
            });
        }
        let value = (percentage.0 / 100.0 * 20.0) as u8;
        TiltSpeed::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FStop;

    #[test]
    fn test_degrees_radians_conversion() {
        let degrees = Degrees(180.0);
        let radians: Radians<f32> = degrees.into();
        assert!((radians.0 - std::f32::consts::PI).abs() < 0.001);

        let radians = Radians(std::f32::consts::PI);
        let degrees: Degrees<f32> = radians.into();
        assert!((degrees.0 - 180.0).abs() < 0.001);
    }

    #[test]
    #[allow(clippy::unwrap_used)] // OK in tests
    fn test_zoom_percentage_conversion() {
        let percentage = Percentage(50.0);
        let zoom = ZoomPosition::try_from(percentage).unwrap();
        assert_eq!(zoom.value(), 0x3800);

        let percentage = Percentage(0.0);
        let zoom = ZoomPosition::try_from(percentage).unwrap();
        assert_eq!(zoom.value(), 0x0000);

        let percentage = Percentage(100.0);
        let zoom = ZoomPosition::try_from(percentage).unwrap();
        assert_eq!(zoom.value(), 0x7000);
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
    #[allow(clippy::unwrap_used)] // OK in tests
    fn test_shutter_fraction_conversion() {
        let fraction = Fraction::new(1, 60);
        let shutter = ShutterSpeed::try_from(fraction).unwrap();
        assert_eq!(shutter.value(), 0x07);

        let fraction = Fraction::new(1, 1000);
        let shutter = ShutterSpeed::try_from(fraction).unwrap();
        assert_eq!(shutter.value(), 0x0C);
    }
}
