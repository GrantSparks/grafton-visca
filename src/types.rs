//! Type-safe wrappers for VISCA protocol values.

use std::fmt;

use crate::error::Error;
use crate::units::Percentage;
use crate::ViscaValue;

// Socket ID - already using ViscaValue efficiently
/// Socket identifier for VISCA command execution slots.
///
/// VISCA cameras maintain two concurrent command execution slots (sockets) to allow
/// up to two commands to be processed simultaneously. Socket 0 is typically used for
/// commands, while Socket 1 is used for inquiries, though this varies by implementation.
///
/// From VISCA protocol: Each camera can process at most two commands concurrently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ViscaValue)]
#[visca_value(valid_values = "[0, 1]", display_prefix = "Socket")]
pub struct SocketId(u8);

impl SocketId {
    /// Socket 0 - First command execution slot.
    pub const SOCKET_0: Self = Self(0);
    /// Socket 1 - Second command execution slot.
    pub const SOCKET_1: Self = Self(1);
}

impl Default for SocketId {
    fn default() -> Self {
        Self::SOCKET_0
    }
}

// Gain types
/// Gain level value for direct gain control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    valid_values = "[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]",
    display_format = "hex",
    display_prefix = "Gain Level",
    model_constraints = "PTZOpticsG2"
)]
pub struct GainLevel(u8);

/// Gain limit value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    valid_values = "[0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xA, 0xB, 0xC, 0xD, 0xE, 0xF]",
    display_format = "hex",
    display_prefix = "Gain Limit",
    model_constraints = "PTZOpticsG2"
)]
pub struct GainLimit(u8);

// Noise reduction levels
/// 2D noise reduction level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "1", max = "5", display_prefix = "2D NR Level")]
pub struct NoiseReduction2DLevel(u8);

/// 3D noise reduction level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "1", max = "8", display_prefix = "3D NR Level")]
pub struct NoiseReduction3DLevel(u8);

// Iris level and conversions
/// Trait for types that can be converted into VISCA iris level values.
///
/// This trait provides a unified conversion interface for different iris-related types
/// (F-stop values, percentages, raw levels) into the VISCA iris level format used
/// for camera aperture control.
pub trait IntoIrisLevel {
    /// Converts this value into a VISCA iris level.
    ///
    /// # Errors
    /// Returns an error if the value cannot be converted to a valid iris level
    /// (e.g., out of range values).
    fn into_iris_level(self) -> Result<IrisLevel, Error>;
}

/// Iris level for direct iris control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    valid_values = "[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C]",
    display_format = "hex",
    display_prefix = "Iris",
    model_constraints = "PTZOpticsG2"
)]
pub struct IrisLevel(u8);

// Macro for implementing IntoIrisLevel conversions
macro_rules! impl_into_iris_level {
    ($($t:ty => $conversion:expr),* $(,)?) => {
        $(
            impl IntoIrisLevel for $t {
                fn into_iris_level(self) -> Result<IrisLevel, Error> {
                    $conversion(self)
                }
            }
        )*
    };
}

impl_into_iris_level! {
    IrisLevel => Ok,
    u8 => IrisLevel::new,
    FStop => |x| Ok(IrisLevel::from(x)),
    Percentage<f32> => |x| {
        use std::convert::TryFrom;
        IrisLevel::try_from(x)
    },
}

impl From<FStop> for IrisLevel {
    fn from(fstop: FStop) -> Self {
        Self(fstop.to_iris_level())
    }
}

// Other camera control types
/// Shutter speed value for direct shutter control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    valid_values = "[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11]",
    display_format = "hex",
    display_prefix = "Shutter",
    model_constraints = "PTZOpticsG2"
)]
pub struct ShutterSpeed(u16);

/// Brightness level for direct brightness control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    valid_values = "[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11]",
    display_format = "hex",
    display_prefix = "Brightness",
    model_constraints = "PTZOpticsG2"
)]
pub struct BrightnessLevel(u16);

// Image adjustment levels
/// Sharpness level for direct sharpness control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    valid_values = "[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B]",
    display_prefix = "Sharpness",
    model_constraints = "PTZOpticsG2"
)]
pub struct SharpnessLevel(u8);

/// Luminance level for brightness adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    valid_values = "[0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xA, 0xB, 0xC, 0xD, 0xE]",
    display_prefix = "Luminance",
    model_constraints = "PTZOpticsG2"
)]
pub struct LuminanceLevel(u8);

/// Contrast level for contrast adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    valid_values = "[0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xA, 0xB, 0xC, 0xD, 0xE]",
    display_prefix = "Contrast",
    model_constraints = "PTZOpticsG2"
)]
pub struct ContrastLevel(u8);

/// Dynamic range level for wide dynamic range control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    valid_values = "[0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8]",
    display_prefix = "Dynamic Range",
    model_constraints = "PTZOpticsG2"
)]
pub struct DynamicRangeLevel(u8);

// Macro for creating speed mapping enums
macro_rules! speed_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident => { pan: $pan:expr, tilt: $tilt:expr, zoom: $zoom:expr, focus: $focus:expr }),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            /// Converts this speed level to a VISCA pan speed value (1-24).
            #[must_use]
            pub fn to_pan_speed(self) -> u8 {
                match self {
                    $(Self::$variant => $pan),*
                }
            }

            /// Converts this speed level to a VISCA tilt speed value (1-20).
            #[must_use]
            pub fn to_tilt_speed(self) -> u8 {
                match self {
                    $(Self::$variant => $tilt),*
                }
            }

            /// Converts this speed level to a VISCA zoom speed value (0-7).
            #[must_use]
            pub fn to_zoom_speed(self) -> u8 {
                match self {
                    $(Self::$variant => $zoom),*
                }
            }

            /// Converts this speed level to a VISCA focus speed value (0-7).
            #[must_use]
            pub fn to_focus_speed(self) -> u8 {
                match self {
                    $(Self::$variant => $focus),*
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::Medium
            }
        }

        impl From<u8> for $name {
            fn from(value: u8) -> Self {
                match value {
                    0..=5 => Self::Slowest,
                    6..=10 => Self::Slow,
                    11..=15 => Self::Medium,
                    16..=20 => Self::Fast,
                    _ => Self::Fastest,
                }
            }
        }
    };
}

speed_enum! {
    /// User-friendly speed level abstraction for camera movements.
    ///
    /// Provides predefined speed combinations that work well across different
    /// camera movement types (pan, tilt, zoom, focus) based on VISCA protocol ranges.
    /// The variants range from `Slowest` (most precise) to `Fastest` (maximum speed).
    pub enum SpeedLevel {
        Slowest => { pan: 1, tilt: 1, zoom: 0, focus: 0 },
        Slow    => { pan: 6, tilt: 5, zoom: 2, focus: 2 },
        Medium  => { pan: 12, tilt: 10, zoom: 4, focus: 4 },
        Fast    => { pan: 18, tilt: 15, zoom: 6, focus: 6 },
        Fastest => { pan: 24, tilt: 20, zoom: 7, focus: 7 },
    }
}

// Macro for F-stop enum with mapping
macro_rules! fstop_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident $(($display:literal))? => $value:expr),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            /// Converts this F-stop value to a VISCA iris level (0x00-0x0C).
            #[must_use]
            pub fn to_iris_level(self) -> u8 {
                match self {
                    $(Self::$variant => $value),*
                }
            }

            /// Creates an F-stop value from a VISCA iris level.
            ///
            /// Returns `None` if the level doesn't correspond to a known F-stop.
            #[must_use]
            pub fn from_iris_level(level: u8) -> Option<Self> {
                match level {
                    $($value => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(
                        Self::$variant => write!(f, fstop_enum!(@display $variant $(, $display)?)),
                    )*
                }
            }
        }
    };

    (@display Closed) => { "Closed" };
    (@display $variant:ident, $display:literal) => { $display };
    (@display $variant:ident) => { stringify!($variant) };
}

fstop_enum! {
    /// F-stop values for iris control.
    ///
    /// Standard VISCA F-stop values from `Closed` (no light) to `F1_8` (widest opening).
    /// These correspond to typical camera aperture settings. Lower numbers mean wider
    /// aperture (more light).
    pub enum FStop {
        Closed => 0x00,
        F11 => 0x01,
        F9_6("F9.6") => 0x02,
        F8 => 0x03,
        F6_8("F6.8") => 0x04,
        F5_6("F5.6") => 0x05,
        F4_8("F4.8") => 0x06,
        F4 => 0x07,
        F3_4("F3.4") => 0x08,
        F2_8("F2.8") => 0x09,
        F2_4("F2.4") => 0x0A,
        F2("F2.0") => 0x0B,
        F1_8("F1.8") => 0x0C,
    }
}

// Noise reduction strength enum
/// User-friendly noise reduction strength levels for VISCA cameras.
///
/// Provides intuitive strength levels that are automatically converted to the
/// appropriate VISCA values for 2D and 3D noise reduction commands. Note that
/// the `Off` variant cannot be used with noise reduction commands as VISCA
/// requires a minimum level (level 1 = minimal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseReductionStrength {
    /// Disable noise reduction (not supported by VISCA - use Minimal instead).
    Off,
    /// Minimal noise reduction with minimal impact on image quality.
    Minimal,
    /// Light noise reduction for moderately noisy environments.
    Light,
    /// Medium noise reduction for noisy environments.
    Medium,
    /// Strong noise reduction for very noisy environments.
    Strong,
    /// Maximum noise reduction for extremely noisy environments.
    Maximum,
}

impl NoiseReductionStrength {
    /// Converts this strength to a VISCA 2D noise reduction level (1-5).
    ///
    /// # Errors
    /// Returns an error for `Off` variant since VISCA 2D noise reduction
    /// requires a minimum level of 1.
    pub fn to_2d_level(self) -> Result<u8, Error> {
        match self {
            Self::Off => Err(Error::InvalidParameter {
                parameter: "strength",
                value: "Off".to_string(),
                reason: "2D noise reduction cannot be turned off, use level 1 for minimal"
                    .to_string(),
            }),
            Self::Minimal => Ok(1),
            Self::Light => Ok(2),
            Self::Medium => Ok(3),
            Self::Strong => Ok(4),
            Self::Maximum => Ok(5),
        }
    }

    /// Converts this strength to a VISCA 3D noise reduction level (1-8).
    ///
    /// # Errors
    /// Returns an error for `Off` variant since VISCA 3D noise reduction
    /// requires a minimum level of 1.
    pub fn to_3d_level(self) -> Result<u8, Error> {
        match self {
            Self::Off => Err(Error::InvalidParameter {
                parameter: "strength",
                value: "Off".to_string(),
                reason: "3D noise reduction cannot be turned off, use level 1 for minimal"
                    .to_string(),
            }),
            Self::Minimal => Ok(1),
            Self::Light => Ok(2),
            Self::Medium => Ok(4),
            Self::Strong => Ok(6),
            Self::Maximum => Ok(8),
        }
    }
}

// Implement TryFrom conversions for noise reduction
impl TryFrom<NoiseReductionStrength> for NoiseReduction2DLevel {
    type Error = Error;
    fn try_from(strength: NoiseReductionStrength) -> Result<Self, Self::Error> {
        Self::new(strength.to_2d_level()?)
    }
}

impl TryFrom<NoiseReductionStrength> for NoiseReduction3DLevel {
    type Error = Error;
    fn try_from(strength: NoiseReductionStrength) -> Result<Self, Self::Error> {
        Self::new(strength.to_3d_level()?)
    }
}

// Zoom and Focus positions with special constants
/// Zoom position value for direct zoom control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    min = "0x0000",
    max = "0x7000",
    display_format = "hex",
    display_prefix = "Zoom"
)]
pub struct ZoomPosition(u16);

impl ZoomPosition {
    /// Maximum optical zoom position (0x4000) as defined by VISCA standard.
    ///
    /// This represents the telephoto end of the optical zoom range for most cameras.
    /// Values beyond this typically engage digital zoom if available.
    pub const MAX_OPTICAL: Self = Self(0x4000);
    /// Maximum digital zoom position (0x7000) for cameras with digital zoom.
    ///
    /// This represents the maximum zoom including both optical and digital zoom.
    /// Not all cameras support digital zoom to this level.
    pub const MAX_DIGITAL: Self = Self(0x7000);
}

/// Focus position value for direct focus control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    min = "0x1000",
    max = "0xF000",
    display_format = "hex",
    display_prefix = "Focus"
)]
pub struct FocusPosition(u16);

// Macro for normalized conversions
macro_rules! impl_normalized_conversion {
    ($type:ty, $min_field:ident, $max_field:ident) => {
        impl TryFrom<f32> for $type {
            type Error = Error;

            fn try_from(normalized: f32) -> Result<Self, Self::Error> {
                if !(0.0..=1.0).contains(&normalized) {
                    return Err(Error::InvalidParameter {
                        parameter: "normalized",
                        value: normalized.to_string(),
                        reason: format!(
                            "Normalized {} must be between 0.0 and 1.0",
                            stringify!($type)
                        ),
                    });
                }
                let range = Self::$max_field.value() - Self::$min_field.value();
                let value =
                    Self::$min_field.value() + (normalized * f32::from(range)).round() as u16;
                Self::new(value)
            }
        }

        impl From<$type> for f32 {
            fn from(pos: $type) -> Self {
                let range = <$type>::$max_field.value() - <$type>::$min_field.value();
                f32::from(pos.value() - <$type>::$min_field.value()) / f32::from(range)
            }
        }
    };
}

impl_normalized_conversion!(ZoomPosition, MIN, MAX_DIGITAL);
impl_normalized_conversion!(FocusPosition, MIN, MAX);

// Color temperature with Kelvin conversion
/// Color temperature value for white balance control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    min = "0x00",
    max = "0x37",
    display_format = "hex",
    display_prefix = "Color Temp"
)]
pub struct ColorTemp(u16);

impl ColorTemp {
    /// Converts this VISCA color temperature value to Kelvin.
    ///
    /// VISCA color temperature range 0x00-0x37 maps to 2500K-8000K.
    #[must_use]
    pub fn to_kelvin(self) -> u16 {
        2500 + (self.0 * 100)
    }

    /// Creates a color temperature value from Kelvin (2500K-8000K).
    ///
    /// # Errors
    /// Returns an error if the Kelvin value is outside the supported range.
    pub fn from_kelvin(kelvin: u16) -> Result<Self, Error> {
        if (2500..=8000).contains(&kelvin) {
            Self::new((kelvin - 2500) / 100)
        } else {
            Err(Error::InvalidParameter {
                parameter: "kelvin",
                value: kelvin.to_string(),
                reason: "Color temperature must be between 2500K and 8000K".to_string(),
            })
        }
    }
}

// Color channels
/// Red gain value for white balance adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    min = "0x00",
    max = "0xFF",
    display_format = "hex",
    display_prefix = "Red Channel"
)]
pub struct RedChannel(u8);

/// Blue gain value for white balance adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(
    min = "0x00",
    max = "0xFF",
    display_format = "hex",
    display_prefix = "Blue Channel"
)]
pub struct BlueChannel(u8);

// Color adjustment levels
/// Saturation level for color adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "0x00", max = "0x0E", display_prefix = "Saturation")]
pub struct SaturationLevel(u8);

impl SaturationLevel {
    /// Converts this saturation level to a percentage value (60%-200%).
    #[must_use]
    pub fn to_percentage(self) -> u8 {
        60 + (self.0 * 10)
    }
}

/// Hue level for color adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "0x00", max = "0x0E", display_prefix = "Hue")]
pub struct HueLevel(u8);

// Use ViscaValue for tuning values
/// Red tuning value for fine white balance adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "-10", max = "10", display_prefix = "Red Tuning")]
pub struct RedTuning(i8);

impl RedTuning {
    /// Neutral red tuning value (no adjustment).
    pub const NEUTRAL: Self = Self(0);
}

/// Blue tuning value for fine white balance adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "-10", max = "10", display_prefix = "Blue Tuning")]
pub struct BlueTuning(i8);

impl BlueTuning {
    /// Neutral blue tuning value (no adjustment).
    pub const NEUTRAL: Self = Self(0);
}

// Use ViscaValue for position types with degree conversion
/// Pan position value for horizontal camera positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "-2448", max = "2448", display_prefix = "Pan")]
pub struct PanPosition(i16);

impl PanPosition {
    /// Center pan position (no horizontal offset).
    pub const CENTER: Self = Self(0);

    /// Converts this pan position to degrees (-170° to +170°).
    #[must_use]
    pub fn to_degrees(self) -> f32 {
        (self.0 as f32) * 170.0 / 2448.0
    }

    /// Creates a pan position from degrees.
    ///
    /// # Errors
    /// Returns an error if degrees are outside the valid range (-170° to +170°).
    pub fn from_degrees(degrees: f32) -> Result<Self, Error> {
        if !(-170.0..=170.0).contains(&degrees) {
            return Err(Error::InvalidParameter {
                parameter: "degrees",
                value: degrees.to_string(),
                reason: "Pan degrees must be between -170° and +170°".to_string(),
            });
        }
        Self::new((degrees * 2448.0 / 170.0).round() as i16)
    }
}

impl TryFrom<f32> for PanPosition {
    type Error = Error;
    fn try_from(degrees: f32) -> Result<Self, Self::Error> {
        Self::from_degrees(degrees)
    }
}

/// Tilt position value for vertical camera positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "-432", max = "1296", display_prefix = "Tilt")]
pub struct TiltPosition(i16);

impl TiltPosition {
    /// Center tilt position (no vertical offset).
    pub const CENTER: Self = Self(0);

    /// Converts this tilt position to degrees.
    ///
    /// Tilt range is asymmetric: -30° (up) to +90° (down).
    #[must_use]
    pub fn to_degrees(self) -> f32 {
        if self.0 >= 0 {
            (self.0 as f32) * 90.0 / 1296.0
        } else {
            (self.0 as f32) * 30.0 / 432.0
        }
    }

    /// Creates a tilt position from degrees.
    ///
    /// # Errors
    /// Returns an error if degrees are outside the valid range (-30° to +90°).
    pub fn from_degrees(degrees: f32) -> Result<Self, Error> {
        if !(-30.0..=90.0).contains(&degrees) {
            return Err(Error::InvalidParameter {
                parameter: "degrees",
                value: degrees.to_string(),
                reason: "Tilt degrees must be between -30° and +90°".to_string(),
            });
        }
        let value = if degrees >= 0.0 {
            (degrees * 1296.0 / 90.0).round() as i16
        } else {
            (degrees * 432.0 / 30.0).round() as i16
        };
        Self::new(value)
    }
}

impl TryFrom<f32> for TiltPosition {
    type Error = Error;
    fn try_from(degrees: f32) -> Result<Self, Self::Error> {
        Self::from_degrees(degrees)
    }
}

// Speed types
/// Pan speed value for horizontal camera movement speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "0x00", max = "0x18", display_prefix = "Pan Speed")]
pub struct PanSpeed(u8);

impl PanSpeed {
    /// Zero pan speed (stopped).
    pub const ZERO: Self = Self(0x00);
}

impl From<SpeedLevel> for PanSpeed {
    fn from(level: SpeedLevel) -> Self {
        Self(level.to_pan_speed())
    }
}

/// Tilt speed value for vertical camera movement speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "0x00", max = "0x14", display_prefix = "Tilt Speed")]
pub struct TiltSpeed(u8);

impl TiltSpeed {
    /// Zero tilt speed (stopped).
    pub const ZERO: Self = Self(0x00);
}

impl From<SpeedLevel> for TiltSpeed {
    fn from(level: SpeedLevel) -> Self {
        Self(level.to_tilt_speed())
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
        assert!(GainLevel::new(0x00).is_ok());
        assert!(GainLevel::new(0x07).is_ok());
        assert!(GainLevel::new(0x08).is_err());

        assert_eq!(GainLevel::MIN.value(), 0x00);
        assert_eq!(GainLevel::MAX.value(), 0x07);
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

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_position_types() {
        // Test pan position
        assert!(PanPosition::new(-2448).is_ok());
        assert!(PanPosition::new(2448).is_ok());
        assert!(PanPosition::new(-2449).is_err());
        assert!(PanPosition::new(2449).is_err());

        // Test degree conversions for pan
        let pan_center = PanPosition::CENTER;
        assert_eq!(pan_center.to_degrees(), 0.0);

        // Test valid degree conversion for pan
        let pan_from_degrees = PanPosition::from_degrees(45.0).unwrap();
        assert!((pan_from_degrees.to_degrees() - 45.0).abs() < 1.0);

        // Test tilt position
        assert!(TiltPosition::new(-432).is_ok());
        assert!(TiltPosition::new(1296).is_ok());
        assert!(TiltPosition::new(-433).is_err());
        assert!(TiltPosition::new(1297).is_err());

        // Test degree conversions for tilt
        let tilt_center = TiltPosition::CENTER;
        assert_eq!(tilt_center.to_degrees(), 0.0);
        let tilt_from_degrees = TiltPosition::from_degrees(45.0).unwrap();
        assert!((tilt_from_degrees.to_degrees() - 45.0).abs() < 1.0);
    }

    #[test]
    fn test_speed_types() {
        // Test pan speed
        assert!(PanSpeed::new(0).is_ok());
        assert!(PanSpeed::new(1).is_ok());
        assert!(PanSpeed::new(24).is_ok());
        assert!(PanSpeed::new(25).is_err());

        // Test tilt speed
        assert!(TiltSpeed::new(0).is_ok());
        assert!(TiltSpeed::new(1).is_ok());
        assert!(TiltSpeed::new(20).is_ok());
        assert!(TiltSpeed::new(21).is_err());

        // Test speed level conversions
        let pan_speed = PanSpeed::from(SpeedLevel::Fast);
        assert_eq!(pan_speed.value(), 18);
        let tilt_speed = TiltSpeed::from(SpeedLevel::Fast);
        assert_eq!(tilt_speed.value(), 15);
    }

    #[test]
    #[allow(clippy::unwrap_used)] // OK in tests
    fn test_ergonomic_conversions() {
        // Test normalized zoom conversions
        let zoom_half = ZoomPosition::try_from(0.5f32).unwrap();
        let normalized: f32 = zoom_half.into();
        assert!((normalized - 0.5).abs() < 0.01);

        // Test normalized focus conversions
        let focus_quarter = FocusPosition::try_from(0.25f32).unwrap();
        let normalized: f32 = focus_quarter.into();
        assert!((normalized - 0.25).abs() < 0.01);

        // Test F-stop to iris level conversion
        let iris = IrisLevel::from(FStop::F2_8);
        assert_eq!(iris.value(), 0x09);

        // Test invalid normalized values
        assert!(ZoomPosition::try_from(-0.1f32).is_err());
        assert!(ZoomPosition::try_from(1.1f32).is_err());
        assert!(FocusPosition::try_from(-0.1f32).is_err());
        assert!(FocusPosition::try_from(1.1f32).is_err());
    }
}
