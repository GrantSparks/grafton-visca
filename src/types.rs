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

/// User-friendly speed level abstraction for camera movements.
///
/// This enum provides intuitive speed names that map to appropriate
/// numeric values for different camera operations (pan, tilt, zoom, focus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedLevel {
    /// Slowest speed - for precise adjustments
    Slowest,
    /// Slow speed
    Slow,
    /// Medium speed - default for most operations
    Medium,
    /// Fast speed
    Fast,
    /// Fastest speed - may cause jerky movements
    Fastest,
}

impl SpeedLevel {
    /// Convert to pan speed value (0-24).
    #[must_use]
    pub fn to_pan_speed(self) -> u8 {
        match self {
            Self::Slowest => 1,
            Self::Slow => 6,
            Self::Medium => 12,
            Self::Fast => 18,
            Self::Fastest => 24,
        }
    }

    /// Convert to tilt speed value (0-20).
    #[must_use]
    pub fn to_tilt_speed(self) -> u8 {
        match self {
            Self::Slowest => 1,
            Self::Slow => 5,
            Self::Medium => 10,
            Self::Fast => 15,
            Self::Fastest => 20,
        }
    }

    /// Convert to zoom speed value (0-7).
    #[must_use]
    pub fn to_zoom_speed(self) -> u8 {
        match self {
            Self::Slowest => 0,
            Self::Slow => 2,
            Self::Medium => 4,
            Self::Fast => 6,
            Self::Fastest => 7,
        }
    }

    /// Convert to focus speed value (0-7).
    #[must_use]
    pub fn to_focus_speed(self) -> u8 {
        match self {
            Self::Slowest => 0,
            Self::Slow => 2,
            Self::Medium => 4,
            Self::Fast => 6,
            Self::Fastest => 7,
        }
    }
}

impl Default for SpeedLevel {
    fn default() -> Self {
        Self::Medium
    }
}

/// F-stop values for iris control.
///
/// Provides named constants for common F-stop values used in camera iris control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FStop {
    /// Iris closed
    Closed,
    /// F11
    F11,
    /// F9.6
    F9_6,
    /// F8
    F8,
    /// F6.8
    F6_8,
    /// F5.6
    F5_6,
    /// F4.8
    F4_8,
    /// F4
    F4,
    /// F3.4
    F3_4,
    /// F2.8
    F2_8,
    /// F2.4
    F2_4,
    /// F2.0
    F2,
    /// F1.8
    F1_8,
}

impl FStop {
    /// Convert to iris level value.
    #[must_use]
    pub fn to_iris_level(self) -> u8 {
        match self {
            Self::Closed => 0x00,
            Self::F11 => 0x01,
            Self::F9_6 => 0x02,
            Self::F8 => 0x03,
            Self::F6_8 => 0x04,
            Self::F5_6 => 0x05,
            Self::F4_8 => 0x06,
            Self::F4 => 0x07,
            Self::F3_4 => 0x08,
            Self::F2_8 => 0x09,
            Self::F2_4 => 0x0A,
            Self::F2 => 0x0B,
            Self::F1_8 => 0x0C,
        }
    }

    /// Create from iris level value.
    pub fn from_iris_level(level: u8) -> Option<Self> {
        match level {
            0x00 => Some(Self::Closed),
            0x01 => Some(Self::F11),
            0x02 => Some(Self::F9_6),
            0x03 => Some(Self::F8),
            0x04 => Some(Self::F6_8),
            0x05 => Some(Self::F5_6),
            0x06 => Some(Self::F4_8),
            0x07 => Some(Self::F4),
            0x08 => Some(Self::F3_4),
            0x09 => Some(Self::F2_8),
            0x0A => Some(Self::F2_4),
            0x0B => Some(Self::F2),
            0x0C => Some(Self::F1_8),
            _ => None,
        }
    }
}

impl fmt::Display for FStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => write!(f, "Closed"),
            Self::F11 => write!(f, "F11"),
            Self::F9_6 => write!(f, "F9.6"),
            Self::F8 => write!(f, "F8"),
            Self::F6_8 => write!(f, "F6.8"),
            Self::F5_6 => write!(f, "F5.6"),
            Self::F4_8 => write!(f, "F4.8"),
            Self::F4 => write!(f, "F4"),
            Self::F3_4 => write!(f, "F3.4"),
            Self::F2_8 => write!(f, "F2.8"),
            Self::F2_4 => write!(f, "F2.4"),
            Self::F2 => write!(f, "F2.0"),
            Self::F1_8 => write!(f, "F1.8"),
        }
    }
}

/// Noise reduction strength levels.
///
/// Provides intuitive names for noise reduction settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseReductionStrength {
    /// Noise reduction disabled
    Off,
    /// Minimal noise reduction
    Minimal,
    /// Light noise reduction
    Light,
    /// Medium noise reduction
    Medium,
    /// Strong noise reduction
    Strong,
    /// Maximum noise reduction
    Maximum,
}

impl NoiseReductionStrength {
    /// Convert to 2D noise reduction level (1-5).
    pub fn to_2d_level(self) -> Result<u8, Error> {
        match self {
            Self::Off => Err(Error::InvalidParameter(
                "2D noise reduction cannot be turned off, use level 1 for minimal".to_string(),
            )),
            Self::Minimal => Ok(1),
            Self::Light => Ok(2),
            Self::Medium => Ok(3),
            Self::Strong => Ok(4),
            Self::Maximum => Ok(5),
        }
    }

    /// Convert to 3D noise reduction level (1-8).
    pub fn to_3d_level(self) -> Result<u8, Error> {
        match self {
            Self::Off => Err(Error::InvalidParameter(
                "3D noise reduction cannot be turned off, use level 1 for minimal".to_string(),
            )),
            Self::Minimal => Ok(1),
            Self::Light => Ok(2),
            Self::Medium => Ok(4),
            Self::Strong => Ok(6),
            Self::Maximum => Ok(8),
        }
    }
}

#[cfg(test)]
mod speed_tests {
    use super::*;

    #[test]
    fn test_speed_level_conversions() {
        assert_eq!(SpeedLevel::Slowest.to_pan_speed(), 1);
        assert_eq!(SpeedLevel::Fastest.to_pan_speed(), 24);

        assert_eq!(SpeedLevel::Slowest.to_tilt_speed(), 1);
        assert_eq!(SpeedLevel::Fastest.to_tilt_speed(), 20);

        assert_eq!(SpeedLevel::Slowest.to_zoom_speed(), 0);
        assert_eq!(SpeedLevel::Fastest.to_zoom_speed(), 7);

        assert_eq!(SpeedLevel::Slowest.to_focus_speed(), 0);
        assert_eq!(SpeedLevel::Fastest.to_focus_speed(), 7);
    }

    #[test]
    fn test_fstop_conversions() {
        assert_eq!(FStop::Closed.to_iris_level(), 0x00);
        assert_eq!(FStop::F1_8.to_iris_level(), 0x0C);

        assert_eq!(FStop::from_iris_level(0x00), Some(FStop::Closed));
        assert_eq!(FStop::from_iris_level(0x0C), Some(FStop::F1_8));
        assert_eq!(FStop::from_iris_level(0xFF), None);
    }

    #[test]
    #[allow(clippy::unwrap_used)] // OK in tests
    fn test_noise_reduction_strength() {
        // Test 2D level conversions
        assert!(NoiseReductionStrength::Off.to_2d_level().is_err());
        assert_eq!(NoiseReductionStrength::Minimal.to_2d_level().unwrap(), 1);
        assert_eq!(NoiseReductionStrength::Maximum.to_2d_level().unwrap(), 5);

        // Test 3D level conversions
        assert!(NoiseReductionStrength::Off.to_3d_level().is_err());
        assert_eq!(NoiseReductionStrength::Minimal.to_3d_level().unwrap(), 1);
        assert_eq!(NoiseReductionStrength::Maximum.to_3d_level().unwrap(), 8);
    }
}

/// Zoom position value for direct zoom control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomPosition(u16);

impl ZoomPosition {
    /// Minimum zoom position (wide).
    pub const MIN: Self = Self(0x0000);

    /// Maximum optical zoom position.
    /// Note: Actual maximum depends on camera model.
    pub const MAX_OPTICAL: Self = Self(0x4000);

    /// Maximum digital zoom position.
    /// Note: Only available if camera supports digital zoom.
    pub const MAX_DIGITAL: Self = Self(0x7000);

    /// Create a new zoom position.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value exceeds 0x7000.
    pub fn new(value: u16) -> Result<Self, Error> {
        if value <= 0x7000 {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "zoom".to_string(),
                value: i32::from(value),
                min: 0x0000,
                max: 0x7000,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for ZoomPosition {
    type Error = Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for ZoomPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Zoom {:#04X}", self.0)
    }
}

/// Focus position value for direct focus control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusPosition(u16);

impl FocusPosition {
    /// Minimum focus position (infinity).
    pub const MIN: Self = Self(0x1000);

    /// Maximum focus position (near).
    /// Note: Actual maximum depends on camera model.
    pub const MAX: Self = Self(0xF000);

    /// Create a new focus position.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside valid range.
    pub fn new(value: u16) -> Result<Self, Error> {
        // Focus position typically uses range 0x1000-0xF000
        if (0x1000..=0xF000).contains(&value) {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "focus".to_string(),
                value: i32::from(value),
                min: 0x1000,
                max: 0xF000,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for FocusPosition {
    type Error = Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for FocusPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Focus {:#04X}", self.0)
    }
}

/// Color temperature value for white balance control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTemperature(u16);

impl ColorTemperature {
    /// Minimum color temperature (2500K).
    pub const MIN: Self = Self(0x00);

    /// Maximum color temperature (8000K).
    pub const MAX: Self = Self(0x37);

    /// Create a new color temperature.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0x00-0x37.
    pub fn new(value: u16) -> Result<Self, Error> {
        if value <= 0x37 {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "color_temperature".to_string(),
                value: i32::from(value),
                min: 0x00,
                max: 0x37,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }

    /// Convert to temperature in Kelvin.
    #[must_use]
    pub fn to_kelvin(self) -> u16 {
        // Linear mapping from 0x00-0x37 to 2500K-8000K
        2500 + (self.0 * 100)
    }

    /// Create from temperature in Kelvin.
    ///
    /// # Errors
    /// Returns `Error::InvalidParameter` if the temperature is outside 2500K-8000K range.
    pub fn from_kelvin(kelvin: u16) -> Result<Self, Error> {
        if (2500..=8000).contains(&kelvin) {
            let value = (kelvin - 2500) / 100;
            Self::new(value)
        } else {
            Err(Error::InvalidParameter(
                "Color temperature must be between 2500K and 8000K".to_string(),
            ))
        }
    }
}

impl TryFrom<u16> for ColorTemperature {
    type Error = Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for ColorTemperature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}K", self.to_kelvin())
    }
}

/// Red gain value for white balance adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedGain(u8);

impl RedGain {
    /// Minimum red gain.
    pub const MIN: Self = Self(0x00);

    /// Maximum red gain.
    pub const MAX: Self = Self(0xFF);

    /// Create a new red gain value.
    ///
    /// # Errors
    /// Never returns an error as all u8 values are valid.
    pub fn new(value: u8) -> Result<Self, Error> {
        Ok(Self(value))
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for RedGain {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for RedGain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Red Gain {:#02X}", self.0)
    }
}

/// Blue gain value for white balance adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlueGain(u8);

impl BlueGain {
    /// Minimum blue gain.
    pub const MIN: Self = Self(0x00);

    /// Maximum blue gain.
    pub const MAX: Self = Self(0xFF);

    /// Create a new blue gain value.
    ///
    /// # Errors
    /// Never returns an error as all u8 values are valid.
    pub fn new(value: u8) -> Result<Self, Error> {
        Ok(Self(value))
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for BlueGain {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for BlueGain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blue Gain {:#02X}", self.0)
    }
}

/// Saturation level for color adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaturationLevel(u8);

impl SaturationLevel {
    /// Minimum saturation (60%).
    pub const MIN: Self = Self(0x00);

    /// Maximum saturation (200%).
    pub const MAX: Self = Self(0x0E);

    /// Create a new saturation level.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0x00-0x0E.
    pub fn new(value: u8) -> Result<Self, Error> {
        if value <= 0x0E {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "saturation".to_string(),
                value: i32::from(value),
                min: 0x00,
                max: 0x0E,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }

    /// Convert to percentage (60% to 200%).
    #[must_use]
    pub fn to_percentage(self) -> u8 {
        60 + (self.0 * 10)
    }
}

impl TryFrom<u8> for SaturationLevel {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for SaturationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Saturation {}%", self.to_percentage())
    }
}

/// Hue level for color adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HueLevel(u8);

impl HueLevel {
    /// Minimum hue level.
    pub const MIN: Self = Self(0x00);

    /// Maximum hue level.
    pub const MAX: Self = Self(0x0E);

    /// Create a new hue level.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside 0x00-0x0E.
    pub fn new(value: u8) -> Result<Self, Error> {
        if value <= 0x0E {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "hue".to_string(),
                value: i32::from(value),
                min: 0x00,
                max: 0x0E,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for HueLevel {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for HueLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hue {}", self.0)
    }
}

/// Red tuning value for fine white balance adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedTuning(i8);

impl RedTuning {
    /// Minimum red tuning.
    pub const MIN: Self = Self(-10);

    /// Maximum red tuning.
    pub const MAX: Self = Self(10);

    /// Create a new red tuning value.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside -10 to +10.
    pub fn new(value: i8) -> Result<Self, Error> {
        if (-10..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "red_tuning".to_string(),
                value: i32::from(value),
                min: -10,
                max: 10,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> i8 {
        self.0
    }
}

impl TryFrom<i8> for RedTuning {
    type Error = Error;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for RedTuning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Red Tuning {:+}", self.0)
    }
}

/// Blue tuning value for fine white balance adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlueTuning(i8);

impl BlueTuning {
    /// Minimum blue tuning.
    pub const MIN: Self = Self(-10);

    /// Maximum blue tuning.
    pub const MAX: Self = Self(10);

    /// Create a new blue tuning value.
    ///
    /// # Errors
    /// Returns `Error::ParameterOutOfRange` if the value is outside -10 to +10.
    pub fn new(value: i8) -> Result<Self, Error> {
        if (-10..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(Error::ParameterOutOfRange {
                parameter: "blue_tuning".to_string(),
                value: i32::from(value),
                min: -10,
                max: 10,
            })
        }
    }

    /// Get the raw value.
    #[must_use]
    pub const fn value(self) -> i8 {
        self.0
    }
}

impl TryFrom<i8> for BlueTuning {
    type Error = Error;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for BlueTuning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blue Tuning {:+}", self.0)
    }
}
