//! Camera profile implementations for specific VISCA camera models.

use std::fmt;

use super::CameraProfile;
use crate::error::Error as ViscaError;

/// PTZOptics G2 camera profile.
#[derive(Debug, Default, Clone, Copy)]
pub struct PTZOpticsG2;

/// PTZOptics 30X camera profile.
#[derive(Debug, Default, Clone, Copy)]
pub struct PTZOptics30X;

/// Sony EVI-D70 camera profile.
#[derive(Debug, Default, Clone, Copy)]
pub struct SonyEVID70;

/// Generic VISCA camera profile for unknown models.
#[derive(Debug, Default, Clone, Copy)]
pub struct GenericVisca;

/// Preset ID for PTZOptics G2 cameras (0-89).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct G2PresetId(u8);

impl G2PresetId {
    /// Create a new preset ID with validation.
    pub fn new(id: u8) -> Result<Self, ViscaError> {
        if id <= 89 {
            Ok(Self(id))
        } else {
            Err(ViscaError::InvalidPreset {
                preset: id,
                max: 89,
            })
        }
    }
}

impl fmt::Display for G2PresetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<G2PresetId> for u8 {
    fn from(preset: G2PresetId) -> Self {
        preset.0
    }
}

impl TryFrom<u8> for G2PresetId {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Gain values for PTZOptics G2 cameras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum G2Gain {
    /// 0dB gain
    Gain0dB = 0,
    /// 3dB gain
    Gain3dB = 1,
    /// 6dB gain
    Gain6dB = 2,
    /// 9dB gain
    Gain9dB = 3,
    /// 12dB gain
    Gain12dB = 4,
    /// 15dB gain
    Gain15dB = 5,
    /// 18dB gain
    Gain18dB = 6,
    /// 21dB gain
    Gain21dB = 7,
    /// 24dB gain
    Gain24dB = 8,
}

impl fmt::Display for G2Gain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = match self {
            G2Gain::Gain0dB => "0dB",
            G2Gain::Gain3dB => "3dB",
            G2Gain::Gain6dB => "6dB",
            G2Gain::Gain9dB => "9dB",
            G2Gain::Gain12dB => "12dB",
            G2Gain::Gain15dB => "15dB",
            G2Gain::Gain18dB => "18dB",
            G2Gain::Gain21dB => "21dB",
            G2Gain::Gain24dB => "24dB",
        };
        write!(f, "{}", db)
    }
}

impl From<G2Gain> for u8 {
    fn from(gain: G2Gain) -> Self {
        gain as u8
    }
}

impl TryFrom<u8> for G2Gain {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(G2Gain::Gain0dB),
            1 => Ok(G2Gain::Gain3dB),
            2 => Ok(G2Gain::Gain6dB),
            3 => Ok(G2Gain::Gain9dB),
            4 => Ok(G2Gain::Gain12dB),
            5 => Ok(G2Gain::Gain15dB),
            6 => Ok(G2Gain::Gain18dB),
            7 => Ok(G2Gain::Gain21dB),
            8 => Ok(G2Gain::Gain24dB),
            _ => Err(ViscaError::InvalidParameter(format!(
                "Invalid gain value: {}",
                value
            ))),
        }
    }
}

impl CameraProfile for PTZOpticsG2 {
    const MODEL_NAME: &'static str = "PTZOptics G2";
    const PAN_RANGE: std::ops::RangeInclusive<i16> = -2448..=2448;
    const TILT_RANGE: std::ops::RangeInclusive<i16> = -432..=1296;
    const ZOOM_RANGE: std::ops::RangeInclusive<u16> = 0x0000..=0x7000;
    const FOCUS_RANGE: std::ops::RangeInclusive<u16> = 0x1000..=0xF000;
    const DIGITAL_ZOOM_SUPPORTED: bool = true;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 20;

    type PresetId = G2PresetId;
    type GainValue = G2Gain;

    fn pan_units_to_degrees(&self, units: i16) -> f32 {
        // G2 has 340° total pan range (-170° to +170°)
        (units as f32 / 2448.0) * 170.0
    }

    fn tilt_units_to_degrees(&self, units: i16) -> f32 {
        // G2 has specific tilt range: -30° to +90° (120° total)
        // -432 units = -30°, 1296 units = +90°
        let normalized = (units + 432) as f32 / (1296 + 432) as f32;
        normalized * 120.0 - 30.0
    }

    fn pan_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees / 170.0 * 2448.0).round() as i16
    }

    fn tilt_degrees_to_units(&self, degrees: f32) -> i16 {
        let normalized = (degrees + 30.0) / 120.0;
        (normalized * (1296 + 432) as f32 - 432.0).round() as i16
    }

    fn max_preset_id() -> u8 {
        89
    }
}

/// Generic preset ID for standard VISCA cameras (0-255).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericPresetId(u8);

impl GenericPresetId {
    /// Create a new preset ID.
    pub fn new(id: u8) -> Self {
        Self(id)
    }
}

impl fmt::Display for GenericPresetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<GenericPresetId> for u8 {
    fn from(preset: GenericPresetId) -> Self {
        preset.0
    }
}

impl TryFrom<u8> for GenericPresetId {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(Self::new(value))
    }
}

/// Generic gain value (0-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericGain(u8);

impl GenericGain {
    /// Create a new gain value with validation.
    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if value <= 15 {
            Ok(Self(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Invalid gain value: {}",
                value
            )))
        }
    }
}

impl fmt::Display for GenericGain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<GenericGain> for u8 {
    fn from(gain: GenericGain) -> Self {
        gain.0
    }
}

impl TryFrom<u8> for GenericGain {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl CameraProfile for GenericVisca {
    const MODEL_NAME: &'static str = "Generic VISCA";
    const PAN_RANGE: std::ops::RangeInclusive<i16> = -32768..=32767;
    const TILT_RANGE: std::ops::RangeInclusive<i16> = -32768..=32767;
    const ZOOM_RANGE: std::ops::RangeInclusive<u16> = 0x0000..=0xFFFF;
    const FOCUS_RANGE: std::ops::RangeInclusive<u16> = 0x0000..=0xFFFF;

    type PresetId = GenericPresetId;
    type GainValue = GenericGain;

    fn pan_units_to_degrees(&self, units: i16) -> f32 {
        // Assume ±180° for generic cameras
        (units as f32 / 32768.0) * 180.0
    }

    fn tilt_units_to_degrees(&self, units: i16) -> f32 {
        // Assume ±90° for generic cameras
        (units as f32 / 32768.0) * 90.0
    }

    fn pan_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees / 180.0 * 32768.0).round() as i16
    }

    fn tilt_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees / 90.0 * 32768.0).round() as i16
    }

    fn max_preset_id() -> u8 {
        255
    }
}

impl CameraProfile for PTZOptics30X {
    const MODEL_NAME: &'static str = "PTZOptics 30X";
    const PAN_RANGE: std::ops::RangeInclusive<i16> = -32768..=32767;
    const TILT_RANGE: std::ops::RangeInclusive<i16> = -20724..=12288;
    const ZOOM_RANGE: std::ops::RangeInclusive<u16> = 0x0000..=0x4000;
    const FOCUS_RANGE: std::ops::RangeInclusive<u16> = 0x1000..=0x8000;
    const MAX_PAN_SPEED: u8 = 18;
    const MAX_TILT_SPEED: u8 = 14;

    type PresetId = GenericPresetId;
    type GainValue = GenericGain;

    fn pan_units_to_degrees(&self, units: i16) -> f32 {
        // 30X has 360° pan range
        (units as f32 / 32768.0) * 180.0
    }

    fn tilt_units_to_degrees(&self, units: i16) -> f32 {
        // 30X specific tilt mapping
        let normalized = (units + 20724) as f32 / (12288 + 20724) as f32;
        normalized * 210.0 - 90.0 // -90° to +120°
    }

    fn pan_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees / 180.0 * 32768.0).round() as i16
    }

    fn tilt_degrees_to_units(&self, degrees: f32) -> i16 {
        let normalized = (degrees + 90.0) / 210.0;
        (normalized * (12288 + 20724) as f32 - 20724.0).round() as i16
    }

    fn max_preset_id() -> u8 {
        255
    }
}

impl CameraProfile for SonyEVID70 {
    const MODEL_NAME: &'static str = "Sony EVI-D70";
    const PAN_RANGE: std::ops::RangeInclusive<i16> = -1440..=1440;
    const TILT_RANGE: std::ops::RangeInclusive<i16> = -360..=360;
    const ZOOM_RANGE: std::ops::RangeInclusive<u16> = 0x0000..=0x4000;
    const FOCUS_RANGE: std::ops::RangeInclusive<u16> = 0x1000..=0xC000;

    type PresetId = GenericPresetId;
    type GainValue = GenericGain;

    fn pan_units_to_degrees(&self, units: i16) -> f32 {
        // EVI-D70: ±100° pan
        (units as f32 / 1440.0) * 100.0
    }

    fn tilt_units_to_degrees(&self, units: i16) -> f32 {
        // EVI-D70: ±25° tilt
        (units as f32 / 360.0) * 25.0
    }

    fn pan_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees / 100.0 * 1440.0).round() as i16
    }

    fn tilt_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees / 25.0 * 360.0).round() as i16
    }

    fn max_preset_id() -> u8 {
        5 // EVI-D70 supports presets 0-5
    }
}
