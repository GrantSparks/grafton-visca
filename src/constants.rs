//! Camera-specific constants and conversion utilities for VISCA protocol.
//!
//! This module provides constants for `PTZOptics` cameras including position ranges,
//! speed limits, and utilities for converting between different unit systems.

use crate::error::ViscaError;

/// `PTZOptics` camera models with their specific capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraModel {
    /// `PTZOptics` G2 series camera
    PTZOpticsG2,
    /// `PTZOptics` G3 series camera
    PTZOpticsG3,
    /// `PTZOptics` 30X optical zoom camera
    PTZOptics30X,
    /// Unknown or generic VISCA camera
    Unknown,
}

/// Pan/Tilt position constants for `PTZOptics` cameras
pub mod position {
    /// Maximum pan position (right) in VISCA units
    pub const PAN_MAX: i16 = 2448;
    /// Minimum pan position (left) in VISCA units
    pub const PAN_MIN: i16 = -2448;
    /// Center pan position
    pub const PAN_CENTER: i16 = 0;

    /// Maximum tilt position (up) in VISCA units
    pub const TILT_MAX: i16 = 1296;
    /// Minimum tilt position (down) in VISCA units
    pub const TILT_MIN: i16 = -432;
    /// Center tilt position
    pub const TILT_CENTER: i16 = 0;

    /// Total pan range in degrees for `PTZOptics` G2 (340°)
    pub const PAN_DEGREES_G2: f32 = 340.0;
    /// Total tilt range in degrees for `PTZOptics` G2 (120°)
    pub const TILT_DEGREES_G2: f32 = 120.0;

    /// Total pan range in degrees for `PTZOptics` 30X (340°)
    pub const PAN_DEGREES_30X: f32 = 340.0;
    /// Total tilt range in degrees for `PTZOptics` 30X (120°)
    pub const TILT_DEGREES_30X: f32 = 120.0;
}

/// Zoom position constants
pub mod zoom {
    /// Minimum zoom position (wide)
    pub const ZOOM_MIN: u16 = 0x0000;
    /// Maximum zoom position for 12X optical zoom
    pub const ZOOM_MAX_12X: u16 = 0x4000;
    /// Maximum zoom position for 20X optical zoom
    pub const ZOOM_MAX_20X: u16 = 0x7000;
    /// Maximum zoom position for 30X optical zoom
    pub const ZOOM_MAX_30X: u16 = 0x7AC0;
    /// Maximum zoom position with digital zoom
    pub const ZOOM_MAX_DIGITAL: u16 = 0x7FFF;
}

/// Focus position constants
pub mod focus {
    /// Minimum focus position (infinity)
    pub const FOCUS_MIN: u16 = 0x1000;
    /// Maximum focus position (near)
    pub const FOCUS_MAX: u16 = 0xF000;
    /// Focus near limit maximum value
    pub const FOCUS_NEAR_LIMIT_MAX: u16 = 0xF000;
}

/// Speed constants for camera movements
pub mod speed {
    /// Maximum pan speed
    pub const PAN_SPEED_MAX: u8 = 0x18; // 24
    /// Maximum tilt speed
    pub const TILT_SPEED_MAX: u8 = 0x14; // 20
    /// Maximum zoom speed
    pub const ZOOM_SPEED_MAX: u8 = 0x07; // 7
    /// Maximum focus speed
    pub const FOCUS_SPEED_MAX: u8 = 0x07; // 7

    /// Default pan speed
    pub const PAN_SPEED_DEFAULT: u8 = 12;
    /// Default tilt speed
    pub const TILT_SPEED_DEFAULT: u8 = 12;
    /// Default zoom speed
    pub const ZOOM_SPEED_DEFAULT: u8 = 4;

    /// Maximum normalized speed (0.0-1.0)
    pub const MAX_PRESET_SPEED: f32 = 1.0;
}

/// Preset constants
pub mod preset {
    /// Minimum preset ID
    pub const PRESET_ID_MIN: u8 = 0;
    /// Maximum preset ID for `PTZOptics` cameras
    pub const PRESET_ID_MAX: u8 = 100;
    /// Home preset ID (usually 0)
    pub const PRESET_HOME: u8 = 0;
}

/// Network constants
pub mod network {
    /// Default VISCA over IP port
    pub const VISCA_DEFAULT_PORT: u16 = 5678;
    /// Secondary VISCA port (some cameras)
    pub const VISCA_SECONDARY_PORT: u16 = 1259;
    /// NDI control port
    pub const NDI_CONTROL_PORT: u16 = 5961;
}

/// Timing constants
pub mod timing {
    /// Default command timeout in milliseconds
    pub const COMMAND_TIMEOUT_MS: u64 = 5000;
    /// Preset recall timeout in milliseconds
    pub const PRESET_RECALL_TIMEOUT_MS: u64 = 10000;
    /// Movement completion check interval in milliseconds
    pub const MOVEMENT_CHECK_INTERVAL_MS: u64 = 100;
    /// Retry delay for failed commands in milliseconds
    pub const RETRY_DELAY_MS: u64 = 100;
}

/// Position represented in VISCA units
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViscaPosition {
    /// Pan position in VISCA units
    pub pan: i16,
    /// Tilt position in VISCA units
    pub tilt: i16,
}

/// Position represented in degrees
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DegreePosition {
    /// Pan position in degrees
    pub pan: f32,
    /// Tilt position in degrees
    pub tilt: f32,
}

/// Position represented as normalized values (-1.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedPosition {
    /// Pan position normalized to -1.0 to 1.0 range
    pub pan: f32,
    /// Tilt position normalized to -1.0 to 1.0 range
    pub tilt: f32,
}

/// Trait for camera-specific constants
pub trait CameraConstants {
    /// Get pan range in VISCA units
    fn pan_range(&self) -> (i16, i16);

    /// Get tilt range in VISCA units
    fn tilt_range(&self) -> (i16, i16);

    /// Get zoom range in VISCA units
    fn zoom_range(&self) -> (u16, u16);

    /// Get pan range in degrees
    fn pan_degrees(&self) -> f32;

    /// Get tilt range in degrees
    fn tilt_degrees(&self) -> f32;

    /// Get maximum pan speed
    fn max_pan_speed(&self) -> u8;

    /// Get maximum tilt speed
    fn max_tilt_speed(&self) -> u8;
}

impl CameraConstants for CameraModel {
    fn pan_range(&self) -> (i16, i16) {
        (position::PAN_MIN, position::PAN_MAX)
    }

    fn tilt_range(&self) -> (i16, i16) {
        (position::TILT_MIN, position::TILT_MAX)
    }

    fn zoom_range(&self) -> (u16, u16) {
        match self {
            Self::PTZOpticsG2 | Self::PTZOpticsG3 => {
                (zoom::ZOOM_MIN, zoom::ZOOM_MAX_20X)
            }
            Self::PTZOptics30X => (zoom::ZOOM_MIN, zoom::ZOOM_MAX_30X),
            Self::Unknown => (zoom::ZOOM_MIN, zoom::ZOOM_MAX_12X),
        }
    }

    fn pan_degrees(&self) -> f32 {
        match self {
            Self::PTZOpticsG2 | Self::PTZOpticsG3 | Self::Unknown => {
                position::PAN_DEGREES_G2
            }
            Self::PTZOptics30X => position::PAN_DEGREES_30X,
        }
    }

    fn tilt_degrees(&self) -> f32 {
        match self {
            Self::PTZOpticsG2 | Self::PTZOpticsG3 | Self::Unknown => {
                position::TILT_DEGREES_G2
            }
            Self::PTZOptics30X => position::TILT_DEGREES_30X,
        }
    }

    fn max_pan_speed(&self) -> u8 {
        speed::PAN_SPEED_MAX
    }

    fn max_tilt_speed(&self) -> u8 {
        speed::TILT_SPEED_MAX
    }
}

/// Trait for position conversion between different unit systems
pub trait PositionConversion {
    /// Convert to normalized position (-1.0 to 1.0)
    fn to_normalized(&self, model: CameraModel) -> NormalizedPosition;

    /// Convert to position in degrees
    fn to_degrees(&self, model: CameraModel) -> DegreePosition;

    /// Convert to VISCA position units
    fn to_visca(&self, model: CameraModel) -> ViscaPosition;
}

impl PositionConversion for ViscaPosition {
    fn to_normalized(&self, model: CameraModel) -> NormalizedPosition {
        let (_pan_min, pan_max) = model.pan_range();
        let (_tilt_min, tilt_max) = model.tilt_range();

        let pan_normalized = f32::from(self.pan) / f32::from(pan_max);
        let tilt_normalized = f32::from(self.tilt) / f32::from(tilt_max);

        NormalizedPosition {
            pan: pan_normalized.clamp(-1.0, 1.0),
            tilt: tilt_normalized.clamp(-1.0, 1.0),
        }
    }

    fn to_degrees(&self, model: CameraModel) -> DegreePosition {
        let (pan_min, pan_max) = model.pan_range();
        let (tilt_min, tilt_max) = model.tilt_range();

        let pan_range = f32::from(pan_max - pan_min);
        let tilt_range = f32::from(tilt_max - tilt_min);

        let pan_ratio = f32::from(self.pan - pan_min) / pan_range;
        let tilt_ratio = f32::from(self.tilt - tilt_min) / tilt_range;

        let pan_degrees = model.pan_degrees();
        let tilt_degrees = model.tilt_degrees();

        DegreePosition {
            pan: pan_ratio.mul_add(pan_degrees, -(pan_degrees / 2.0)),
            tilt: tilt_ratio.mul_add(tilt_degrees, -(tilt_degrees / 2.0)),
        }
    }

    fn to_visca(&self, _model: CameraModel) -> ViscaPosition {
        *self
    }
}

impl PositionConversion for DegreePosition {
    fn to_normalized(&self, model: CameraModel) -> NormalizedPosition {
        let pan_degrees = model.pan_degrees();
        let tilt_degrees = model.tilt_degrees();

        NormalizedPosition {
            pan: (self.pan / (pan_degrees / 2.0)).clamp(-1.0, 1.0),
            tilt: (self.tilt / (tilt_degrees / 2.0)).clamp(-1.0, 1.0),
        }
    }

    fn to_degrees(&self, _model: CameraModel) -> DegreePosition {
        *self
    }

    fn to_visca(&self, model: CameraModel) -> ViscaPosition {
        let (pan_min, pan_max) = model.pan_range();
        let (tilt_min, tilt_max) = model.tilt_range();

        let pan_degrees = model.pan_degrees();
        let tilt_degrees = model.tilt_degrees();

        let pan_ratio = (self.pan + (pan_degrees / 2.0)) / pan_degrees;
        let tilt_ratio = (self.tilt + (tilt_degrees / 2.0)) / tilt_degrees;

        let pan_range = f32::from(pan_max - pan_min);
        let tilt_range = f32::from(tilt_max - tilt_min);

        let pan = pan_ratio.mul_add(pan_range, f32::from(pan_min)).round() as i16;
        let tilt = tilt_ratio.mul_add(tilt_range, f32::from(tilt_min)).round() as i16;
        
        ViscaPosition { pan, tilt }
    }
}

impl PositionConversion for NormalizedPosition {
    fn to_normalized(&self, _model: CameraModel) -> NormalizedPosition {
        *self
    }

    fn to_degrees(&self, model: CameraModel) -> DegreePosition {
        let pan_degrees = model.pan_degrees();
        let tilt_degrees = model.tilt_degrees();

        DegreePosition {
            pan: self.pan * (pan_degrees / 2.0),
            tilt: self.tilt * (tilt_degrees / 2.0),
        }
    }

    fn to_visca(&self, model: CameraModel) -> ViscaPosition {
        let (pan_min, pan_max) = model.pan_range();
        let (tilt_min, tilt_max) = model.tilt_range();

        let pan = if self.pan >= 0.0 {
            (self.pan * f32::from(pan_max)).round() as i16
        } else {
            (self.pan * f32::from(-pan_min)).round() as i16
        };

        let tilt = if self.tilt >= 0.0 {
            (self.tilt * f32::from(tilt_max)).round() as i16
        } else {
            (self.tilt * f32::from(-tilt_min)).round() as i16
        };

        ViscaPosition { pan, tilt }
    }
}

/// Validate pan position is within camera limits
///
/// # Errors
///
/// Returns `ViscaError::ParameterOutOfRange` if the pan position is outside the valid range for the camera model
pub fn validate_pan_position(pos: i16, model: CameraModel) -> Result<i16, ViscaError> {
    let (min, max) = model.pan_range();
    if pos < min || pos > max {
        Err(ViscaError::ParameterOutOfRange {
            parameter: "pan".to_string(),
            value: i32::from(pos),
            min: i32::from(min),
            max: i32::from(max),
        })
    } else {
        Ok(pos)
    }
}

/// Validate tilt position is within camera limits
///
/// # Errors
///
/// Returns `ViscaError::ParameterOutOfRange` if the tilt position is outside the valid range for the camera model
pub fn validate_tilt_position(pos: i16, model: CameraModel) -> Result<i16, ViscaError> {
    let (min, max) = model.tilt_range();
    if pos < min || pos > max {
        Err(ViscaError::ParameterOutOfRange {
            parameter: "tilt".to_string(),
            value: i32::from(pos),
            min: i32::from(min),
            max: i32::from(max),
        })
    } else {
        Ok(pos)
    }
}

/// Validate zoom position is within camera limits
///
/// # Errors
///
/// Returns `ViscaError::ParameterOutOfRange` if the zoom position is outside the valid range for the camera model
pub fn validate_zoom_position(pos: u16, model: CameraModel) -> Result<u16, ViscaError> {
    let (min, max) = model.zoom_range();
    if pos < min || pos > max {
        Err(ViscaError::ParameterOutOfRange {
            parameter: "zoom".to_string(),
            value: i32::from(pos),
            min: i32::from(min),
            max: i32::from(max),
        })
    } else {
        Ok(pos)
    }
}

/// Validate pan speed is within limits
///
/// # Errors
///
/// Returns `ViscaError::ParameterOutOfRange` if the pan speed exceeds the maximum allowed value
pub fn validate_pan_speed(speed: u8) -> Result<u8, ViscaError> {
    if speed > speed::PAN_SPEED_MAX {
        Err(ViscaError::ParameterOutOfRange {
            parameter: "pan_speed".to_string(),
            value: i32::from(speed),
            min: 0,
            max: i32::from(speed::PAN_SPEED_MAX),
        })
    } else {
        Ok(speed)
    }
}

/// Validate tilt speed is within limits
///
/// # Errors
///
/// Returns `ViscaError::ParameterOutOfRange` if the tilt speed exceeds the maximum allowed value
pub fn validate_tilt_speed(speed: u8) -> Result<u8, ViscaError> {
    if speed > speed::TILT_SPEED_MAX {
        Err(ViscaError::ParameterOutOfRange {
            parameter: "tilt_speed".to_string(),
            value: i32::from(speed),
            min: 0,
            max: i32::from(speed::TILT_SPEED_MAX),
        })
    } else {
        Ok(speed)
    }
}

/// Validate preset ID is within limits
///
/// # Errors
///
/// Returns `ViscaError::ParameterOutOfRange` if the preset ID is outside the valid range
pub fn validate_preset_id(id: u8) -> Result<u8, ViscaError> {
    if id > preset::PRESET_ID_MAX {
        Err(ViscaError::ParameterOutOfRange {
            parameter: "preset_id".to_string(),
            value: i32::from(id),
            min: i32::from(preset::PRESET_ID_MIN),
            max: i32::from(preset::PRESET_ID_MAX),
        })
    } else {
        Ok(id)
    }
}

// Standalone conversion functions for ease of use
// These provide a simpler API without requiring CameraModel

/// Convert normalized pan value (-1.0 to 1.0) to VISCA units
#[inline]
#[must_use]
pub fn pan_normalized_to_visca(normalized: f32) -> i16 {
    (normalized.clamp(-1.0, 1.0) * f32::from(position::PAN_MAX)).round() as i16
}

/// Convert VISCA pan units to normalized value (-1.0 to 1.0)
#[inline]
#[must_use]
pub fn pan_visca_to_normalized(visca: i16) -> f32 {
    f32::from(visca) / f32::from(position::PAN_MAX)
}

/// Convert pan degrees to VISCA units (assumes G2 camera)
#[inline]
#[must_use]
pub fn pan_degrees_to_visca(degrees: f32) -> i16 {
    let normalized = degrees / (position::PAN_DEGREES_G2 / 2.0);
    pan_normalized_to_visca(normalized)
}

/// Convert VISCA pan units to degrees (assumes G2 camera)
#[inline]
#[must_use]
pub fn pan_visca_to_degrees(visca: i16) -> f32 {
    let normalized = pan_visca_to_normalized(visca);
    normalized * (position::PAN_DEGREES_G2 / 2.0)
}

/// Convert tilt normalized value (-1.0 to 1.0) to VISCA units
#[inline]
#[must_use]
pub fn tilt_normalized_to_visca(normalized: f32) -> i16 {
    (normalized.clamp(-1.0, 1.0) * f32::from(position::TILT_MAX)).round() as i16
}

/// Convert VISCA tilt units to normalized value (-1.0 to 1.0)
#[inline]
#[must_use]
pub fn tilt_visca_to_normalized(visca: i16) -> f32 {
    f32::from(visca) / f32::from(position::TILT_MAX)
}

/// Convert tilt degrees to VISCA units (assumes G2 camera)
#[inline]
#[must_use]
pub fn tilt_degrees_to_visca(degrees: f32) -> i16 {
    let normalized = degrees / (position::TILT_DEGREES_G2 / 2.0);
    tilt_normalized_to_visca(normalized)
}

/// Convert VISCA tilt units to degrees (assumes G2 camera)
#[inline]
#[must_use]
pub fn tilt_visca_to_degrees(visca: i16) -> f32 {
    let normalized = tilt_visca_to_normalized(visca);
    normalized * (position::TILT_DEGREES_G2 / 2.0)
}

/// Convert zoom normalized value (0.0 to 1.0) to VISCA units
#[inline]
#[must_use]
pub fn zoom_normalized_to_visca(normalized: f32) -> u16 {
    (normalized.clamp(0.0, 1.0) * f32::from(zoom::ZOOM_MAX_20X)).round() as u16
}

/// Convert VISCA zoom units to normalized value (0.0 to 1.0)
#[inline]
#[must_use]
pub fn zoom_visca_to_normalized(visca: u16) -> f32 {
    f32::from(visca) / f32::from(zoom::ZOOM_MAX_20X)
}

/// Convert zoom magnification (1x to 20x) to VISCA units
#[inline]
#[must_use]
pub fn zoom_magnification_to_visca(magnification: f32) -> u16 {
    let normalized = (magnification.clamp(1.0, 20.0) - 1.0) / 19.0;
    zoom_normalized_to_visca(normalized)
}

/// Convert VISCA zoom units to magnification (1x to 20x)
#[inline]
#[must_use]
pub fn zoom_visca_to_magnification(visca: u16) -> f32 {
    let normalized = zoom_visca_to_normalized(visca);
    normalized.mul_add(19.0, 1.0)
}

/// Convert focus normalized value (0.0 to 1.0) to VISCA units
#[inline]
#[must_use]
pub fn focus_normalized_to_visca(normalized: f32) -> u16 {
    let range = f32::from(focus::FOCUS_MAX - focus::FOCUS_MIN);
    focus::FOCUS_MIN + (normalized.clamp(0.0, 1.0) * range).round() as u16
}

/// Convert VISCA focus units to normalized value (0.0 to 1.0)
#[inline]
#[must_use]
pub fn focus_visca_to_normalized(visca: u16) -> f32 {
    let clamped = visca.clamp(focus::FOCUS_MIN, focus::FOCUS_MAX);
    let range = f32::from(focus::FOCUS_MAX - focus::FOCUS_MIN);
    f32::from(clamped - focus::FOCUS_MIN) / range
}

/// Convert speed normalized value (0.0 to 1.0) to VISCA pan speed units
#[inline]
#[must_use]
pub fn pan_speed_normalized_to_visca(normalized: f32) -> u8 {
    (normalized.clamp(0.0, 1.0) * f32::from(speed::PAN_SPEED_MAX)).round() as u8
}

/// Convert speed normalized value (0.0 to 1.0) to VISCA tilt speed units
#[inline]
#[must_use]
pub fn tilt_speed_normalized_to_visca(normalized: f32) -> u8 {
    (normalized.clamp(0.0, 1.0) * f32::from(speed::TILT_SPEED_MAX)).round() as u8
}

/// Convert speed normalized value (0.0 to 1.0) to VISCA zoom speed units
#[inline]
#[must_use]
pub fn zoom_speed_normalized_to_visca(normalized: f32) -> u8 {
    (normalized.clamp(0.0, 1.0) * f32::from(speed::ZOOM_SPEED_MAX)).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visca_to_degrees_conversion() {
        // Test center position
        let visca_pos = ViscaPosition { pan: 0, tilt: 432 };
        let degrees = visca_pos.to_degrees(CameraModel::PTZOpticsG2);

        // Pan 0 should map to 0 degrees (center)
        assert!((degrees.pan - 0.0).abs() < 0.1);
        // Tilt 432 is (432 - (-432)) / (1296 - (-432)) = 864/1728 = 0.5 of range
        // 0.5 * 120 - 60 = 0 degrees
        assert!((degrees.tilt - 0.0).abs() < 1.0);

        // Test maximum positions
        let visca_pos = ViscaPosition {
            pan: position::PAN_MAX,
            tilt: position::TILT_MAX,
        };
        let degrees = visca_pos.to_degrees(CameraModel::PTZOpticsG2);

        assert!((degrees.pan - 170.0).abs() < 1.0); // Half of 340 degrees
        assert!((degrees.tilt - 60.0).abs() < 1.0); // Maximum tilt
    }

    #[test]
    fn test_degrees_to_visca_conversion() {
        let degree_pos = DegreePosition {
            pan: 0.0,
            tilt: 0.0,
        };
        let visca = degree_pos.to_visca(CameraModel::PTZOpticsG2);

        // 0 degrees pan should map to VISCA 0
        assert_eq!(visca.pan, 0);
        // 0 degrees tilt should map to middle of the range
        let tilt_middle = (position::TILT_MAX + position::TILT_MIN) / 2;
        assert!((visca.tilt - tilt_middle).abs() < 100);
    }

    #[test]
    fn test_normalized_conversion() {
        let norm_pos = NormalizedPosition {
            pan: 1.0,
            tilt: 1.0,
        };
        let visca = norm_pos.to_visca(CameraModel::PTZOpticsG2);

        assert_eq!(visca.pan, position::PAN_MAX);
        assert_eq!(visca.tilt, position::TILT_MAX);

        let norm_pos = NormalizedPosition {
            pan: -1.0,
            tilt: -1.0,
        };
        let visca = norm_pos.to_visca(CameraModel::PTZOpticsG2);

        assert_eq!(visca.pan, position::PAN_MIN);
        assert_eq!(visca.tilt, position::TILT_MIN);
    }

    #[test]
    fn test_validation_functions() {
        assert!(validate_pan_position(0, CameraModel::PTZOpticsG2).is_ok());
        assert!(validate_pan_position(5000, CameraModel::PTZOpticsG2).is_err());

        assert!(validate_tilt_position(0, CameraModel::PTZOpticsG2).is_ok());
        assert!(validate_tilt_position(-1000, CameraModel::PTZOpticsG2).is_err());

        assert!(validate_pan_speed(12).is_ok());
        assert!(validate_pan_speed(30).is_err());

        assert!(validate_preset_id(50).is_ok());
        assert!(validate_preset_id(150).is_err());
    }

    #[test]
    fn test_standalone_pan_conversions() {
        // Test normalized to VISCA
        assert_eq!(pan_normalized_to_visca(1.0), position::PAN_MAX);
        assert_eq!(pan_normalized_to_visca(-1.0), -position::PAN_MAX);
        assert_eq!(pan_normalized_to_visca(0.0), 0);
        assert_eq!(pan_normalized_to_visca(0.5), position::PAN_MAX / 2);

        // Test VISCA to normalized
        assert_eq!(pan_visca_to_normalized(position::PAN_MAX), 1.0);
        assert_eq!(pan_visca_to_normalized(-position::PAN_MAX), -1.0);
        assert_eq!(pan_visca_to_normalized(0), 0.0);

        // Test degrees to VISCA
        assert_eq!(pan_degrees_to_visca(0.0), 0);
        assert_eq!(pan_degrees_to_visca(170.0), position::PAN_MAX);
        assert_eq!(pan_degrees_to_visca(-170.0), -position::PAN_MAX);

        // Test VISCA to degrees
        assert!((pan_visca_to_degrees(0) - 0.0).abs() < 0.1);
        assert!((pan_visca_to_degrees(position::PAN_MAX) - 170.0).abs() < 0.1);
    }

    #[test]
    fn test_standalone_tilt_conversions() {
        // Test normalized to VISCA
        assert_eq!(tilt_normalized_to_visca(1.0), position::TILT_MAX);
        assert_eq!(tilt_normalized_to_visca(-1.0), -position::TILT_MAX);
        assert_eq!(tilt_normalized_to_visca(0.0), 0);

        // Test VISCA to normalized
        assert_eq!(tilt_visca_to_normalized(position::TILT_MAX), 1.0);
        assert_eq!(tilt_visca_to_normalized(0), 0.0);

        // Test degrees to VISCA
        assert_eq!(tilt_degrees_to_visca(0.0), 0);
        assert_eq!(tilt_degrees_to_visca(60.0), position::TILT_MAX);
        assert_eq!(tilt_degrees_to_visca(-60.0), -position::TILT_MAX);
    }

    #[test]
    fn test_standalone_zoom_conversions() {
        // Test normalized to VISCA
        assert_eq!(zoom_normalized_to_visca(0.0), 0);
        assert_eq!(zoom_normalized_to_visca(1.0), zoom::ZOOM_MAX_20X);
        assert_eq!(zoom_normalized_to_visca(0.5), zoom::ZOOM_MAX_20X / 2);

        // Test VISCA to normalized
        assert_eq!(zoom_visca_to_normalized(0), 0.0);
        assert_eq!(zoom_visca_to_normalized(zoom::ZOOM_MAX_20X), 1.0);

        // Test magnification conversions
        assert_eq!(zoom_magnification_to_visca(1.0), 0); // 1x = minimum zoom
        assert_eq!(zoom_magnification_to_visca(20.0), zoom::ZOOM_MAX_20X); // 20x = maximum
        assert!((zoom_visca_to_magnification(0) - 1.0).abs() < 0.1);
        assert!((zoom_visca_to_magnification(zoom::ZOOM_MAX_20X) - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_standalone_focus_conversions() {
        // Test normalized to VISCA
        assert_eq!(focus_normalized_to_visca(0.0), focus::FOCUS_MIN);
        assert_eq!(focus_normalized_to_visca(1.0), focus::FOCUS_MAX);

        // Test VISCA to normalized
        assert_eq!(focus_visca_to_normalized(focus::FOCUS_MIN), 0.0);
        assert_eq!(focus_visca_to_normalized(focus::FOCUS_MAX), 1.0);
    }

    #[test]
    fn test_speed_conversions() {
        // Test pan speed
        assert_eq!(pan_speed_normalized_to_visca(0.0), 0);
        assert_eq!(pan_speed_normalized_to_visca(1.0), speed::PAN_SPEED_MAX);
        assert_eq!(pan_speed_normalized_to_visca(0.5), speed::PAN_SPEED_MAX / 2);

        // Test tilt speed
        assert_eq!(tilt_speed_normalized_to_visca(0.0), 0);
        assert_eq!(tilt_speed_normalized_to_visca(1.0), speed::TILT_SPEED_MAX);

        // Test zoom speed
        assert_eq!(zoom_speed_normalized_to_visca(0.0), 0);
        assert_eq!(zoom_speed_normalized_to_visca(1.0), speed::ZOOM_SPEED_MAX);
    }
}
