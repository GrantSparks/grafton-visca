//! Type-safe wrappers for VISCA protocol values.
//!
//! This module provides newtype wrappers for various VISCA protocol values
//! to ensure type safety and prevent invalid states at compile time.

use std::fmt;

use crate::error::Error;

/// Socket ID for VISCA commands.
///
/// VISCA protocol uses socket IDs 0 or 1 to track command execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketId(u8);

impl SocketId {
    /// Socket ID 0.
    pub const SOCKET_0: Self = Self(0);

    /// Socket ID 1.
    pub const SOCKET_1: Self = Self(1);

    /// Create a new socket ID.
    ///
    /// # Errors
    /// Returns `Error::InvalidParameter` if the value is not 0 or 1.
    pub fn new(value: u8) -> Result<Self, Error> {
        match value {
            0 | 1 => Ok(Self(value)),
            _ => Err(Error::InvalidParameter(format!(
                "Socket ID must be 0 or 1, got {value}"
            ))),
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl fmt::Display for SocketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Socket {}", self.0)
    }
}

impl Default for SocketId {
    fn default() -> Self {
        Self::SOCKET_0
    }
}

/// Gain value for direct gain control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GainValue(u8);

impl GainValue {
    /// Minimum gain value.
    pub const MIN: Self = Self(0x00);

    /// Maximum gain value.
    pub const MAX: Self = Self(0x07);

    /// Valid gain values for `PTZOptics` G2 cameras.
    /// These are the only valid values according to the G2 specification.
    pub const G2_VALID_VALUES: &'static [u8] = &[
        0x00, // 0dB
        0x01, // 3dB
        0x02, // 6dB
        0x03, // 9dB
        0x04, // 12dB
        0x05, // 15dB
        0x06, // 18dB
        0x07, // 21dB
    ];

    /// Create a new gain value.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0x00-0x07.
    pub fn new(value: u8) -> Result<Self, Error> {
        if value <= 0x07 {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "gain".to_string(),
                value: i32::from(value),
                min: 0x00,
                max: 0x07,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for GainValue {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for GainValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Gain {:#02X}", self.0)
    }
}

/// Gain limit value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GainLimit(u8);

impl GainLimit {
    /// Minimum gain limit.
    pub const MIN: Self = Self(0x0);

    /// Maximum gain limit.
    pub const MAX: Self = Self(0xF);

    /// Valid gain limit values for G2 cameras (0x0-0xF).
    pub const G2_VALID_VALUES: &'static [u8] = &[
        0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xA, 0xB, 0xC, 0xD, 0xE, 0xF,
    ];

    /// Create a new gain limit.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0x0-0xF.
    pub fn new(value: u8) -> Result<Self, Error> {
        if value <= 0xF {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "gain_limit".to_string(),
                value: i32::from(value),
                min: 0x0,
                max: 0xF,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for GainLimit {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for GainLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Gain Limit {:#X}", self.0)
    }
}

/// 2D noise reduction level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseReduction2DLevel(u8);

impl NoiseReduction2DLevel {
    /// Minimum 2D noise reduction level.
    pub const MIN: Self = Self(1);

    /// Maximum 2D noise reduction level.
    pub const MAX: Self = Self(5);

    /// Create a new 2D noise reduction level.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 1-5.
    pub fn new(value: u8) -> Result<Self, Error> {
        match value {
            1..=5 => Ok(Self(value)),
            _ => Err(Error::ParameterOutOfRange {
                parameter: "2d_noise_reduction".to_string(),
                value: i32::from(value),
                min: 1,
                max: 5,
            }),
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for NoiseReduction2DLevel {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for NoiseReduction2DLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "2D NR Level {}", self.0)
    }
}

/// 3D noise reduction level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseReduction3DLevel(u8);

impl NoiseReduction3DLevel {
    /// Minimum 3D noise reduction level.
    pub const MIN: Self = Self(1);

    /// Maximum 3D noise reduction level.
    pub const MAX: Self = Self(8);

    /// Create a new 3D noise reduction level.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 1-8.
    pub fn new(value: u8) -> Result<Self, Error> {
        match value {
            1..=8 => Ok(Self(value)),
            _ => Err(Error::ParameterOutOfRange {
                parameter: "3d_noise_reduction".to_string(),
                value: i32::from(value),
                min: 1,
                max: 8,
            }),
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for NoiseReduction3DLevel {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for NoiseReduction3DLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "3D NR Level {}", self.0)
    }
}

/// Iris level for direct iris control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrisLevel(u8);

impl IrisLevel {
    /// Minimum iris level (fully closed).
    pub const MIN: Self = Self(0x00);

    /// Maximum iris level (F1.8 - fully open).
    pub const MAX: Self = Self(0x0C);

    /// Valid iris level values for `PTZOptics` G2 cameras.
    /// These are the only valid values according to the G2 specification.
    pub const G2_VALID_VALUES: &'static [u8] = &[
        0x00, // Close
        0x01, // F11
        0x02, // F9.6
        0x03, // F8
        0x04, // F6.8
        0x05, // F5.6
        0x06, // F4.8
        0x07, // F4
        0x08, // F3.4
        0x09, // F2.8
        0x0A, // F2.4
        0x0B, // F2
        0x0C, // F1.8
    ];

    /// Create a new iris level.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0x00-0x0C.
    pub fn new(value: u8) -> Result<Self, Error> {
        if value <= 0x0C {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "iris".to_string(),
                value: i32::from(value),
                min: 0x00,
                max: 0x0C,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for IrisLevel {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for IrisLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Iris {:#02X}", self.0)
    }
}

/// Shutter speed value for direct shutter control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutterSpeed(u16);

impl ShutterSpeed {
    /// Minimum shutter speed (1/30 second).
    pub const MIN: Self = Self(0x01);

    /// Maximum shutter speed (1/10000 second).
    pub const MAX: Self = Self(0x11);

    /// Valid shutter speed values for `PTZOptics` G2 cameras.
    /// These are the only valid values according to the G2 specification.
    pub const G2_VALID_VALUES: &'static [u16] = &[
        0x01, // 1/30
        0x02, // 1/60
        0x03, // 1/100
        0x04, // 1/125
        0x05, // 1/180
        0x06, // 1/250
        0x07, // 1/350
        0x08, // 1/500
        0x09, // 1/725
        0x0A, // 1/1000
        0x0B, // 1/1500
        0x0C, // 1/2000
        0x0D, // 1/3000
        0x0E, // 1/4000
        0x0F, // 1/6000
        0x10, // 1/8000
        0x11, // 1/10000
    ];

    /// Create a new shutter speed.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0x01-0x11.
    pub fn new(value: u16) -> Result<Self, Error> {
        if (0x01..=0x11).contains(&value) {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "shutter".to_string(),
                value: i32::from(value),
                min: 0x01,
                max: 0x11,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for ShutterSpeed {
    type Error = Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for ShutterSpeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Shutter {:#04X}", self.0)
    }
}

/// Brightness level for direct brightness control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrightnessLevel(u16);

impl BrightnessLevel {
    /// Minimum brightness level.
    pub const MIN: Self = Self(0x00);

    /// Maximum brightness level.
    pub const MAX: Self = Self(0x11);

    /// Valid brightness values for G2 cameras (0x00-0x11).
    pub const G2_VALID_VALUES: &'static [u16] = &[
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        0x0F, 0x10, 0x11,
    ];

    /// Create a new brightness level.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0x00-0x11.
    pub fn new(value: u16) -> Result<Self, Error> {
        if value <= 0x11 {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "brightness".to_string(),
                value: i32::from(value),
                min: 0x00,
                max: 0x11,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for BrightnessLevel {
    type Error = Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for BrightnessLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Brightness {:#04X}", self.0)
    }
}

/// Sharpness level for direct sharpness control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharpnessLevel(u8);

impl SharpnessLevel {
    /// Minimum sharpness level.
    pub const MIN: Self = Self(0);

    /// Maximum sharpness level.
    pub const MAX: Self = Self(11);

    /// Valid sharpness values for `PTZOptics` G2 cameras.
    /// These are the only valid values according to the G2 specification.
    pub const G2_VALID_VALUES: &'static [u8] = &[
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
    ];

    /// Create a new sharpness level.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0-11.
    pub fn new(value: u8) -> Result<Self, Error> {
        if value <= 11 {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "sharpness".to_string(),
                value: i32::from(value),
                min: 0,
                max: 11,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for SharpnessLevel {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for SharpnessLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sharpness {}", self.0)
    }
}

/// Luminance level for brightness adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuminanceLevel(u8);

impl LuminanceLevel {
    /// Minimum luminance level.
    pub const MIN: Self = Self(0);

    /// Maximum luminance level.
    pub const MAX: Self = Self(14);

    /// Valid luminance values for G2 cameras (0x0-0xE).
    pub const G2_VALID_VALUES: &'static [u8] = &[
        0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xA, 0xB, 0xC, 0xD, 0xE,
    ];

    /// Create a new luminance level.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0-14.
    pub fn new(value: u8) -> Result<Self, Error> {
        if value <= 14 {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "luminance".to_string(),
                value: i32::from(value),
                min: 0,
                max: 14,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for LuminanceLevel {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for LuminanceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Luminance {}", self.0)
    }
}

/// Contrast level for contrast adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContrastLevel(u8);

impl ContrastLevel {
    /// Minimum contrast level.
    pub const MIN: Self = Self(0);

    /// Maximum contrast level.
    pub const MAX: Self = Self(14);

    /// Valid contrast values for G2 cameras (0x0-0xE).
    pub const G2_VALID_VALUES: &'static [u8] = &[
        0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xA, 0xB, 0xC, 0xD, 0xE,
    ];

    /// Create a new contrast level.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0-14.
    pub fn new(value: u8) -> Result<Self, Error> {
        if value <= 14 {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "contrast".to_string(),
                value: i32::from(value),
                min: 0,
                max: 14,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for ContrastLevel {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for ContrastLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Contrast {}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_id() {
        assert!(SocketId::new(0).is_ok());
        assert!(SocketId::new(1).is_ok());
        assert!(SocketId::new(2).is_err());

        assert_eq!(SocketId::SOCKET_0.value(), 0);
        assert_eq!(SocketId::SOCKET_1.value(), 1);
        assert_eq!(SocketId::default(), SocketId::SOCKET_0);
    }

    #[test]
    fn test_gain_value() {
        assert!(GainValue::new(0x00).is_ok());
        assert!(GainValue::new(0x07).is_ok());
        assert!(GainValue::new(0x08).is_err());

        assert_eq!(GainValue::MIN.value(), 0x00);
        assert_eq!(GainValue::MAX.value(), 0x07);
    }

    #[test]
    fn test_gain_limit() {
        assert!(GainLimit::new(0x0).is_ok());
        assert!(GainLimit::new(0xF).is_ok());
        assert!(GainLimit::new(0x10).is_err());

        assert_eq!(GainLimit::MIN.value(), 0x0);
        assert_eq!(GainLimit::MAX.value(), 0xF);
    }

    #[test]
    fn test_noise_reduction_levels() {
        // 2D noise reduction
        assert!(NoiseReduction2DLevel::new(0).is_err());
        assert!(NoiseReduction2DLevel::new(1).is_ok());
        assert!(NoiseReduction2DLevel::new(5).is_ok());
        assert!(NoiseReduction2DLevel::new(6).is_err());

        // 3D noise reduction
        assert!(NoiseReduction3DLevel::new(0).is_err());
        assert!(NoiseReduction3DLevel::new(1).is_ok());
        assert!(NoiseReduction3DLevel::new(8).is_ok());
        assert!(NoiseReduction3DLevel::new(9).is_err());
    }
}
