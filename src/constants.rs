//! Internal camera-specific validation constants for VISCA value types.

use crate::error::Error;

/// Camera variants for model-aware value validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraVariant {
    /// `PtzOptics` G2 series camera
    PtzOpticsG2,
    /// `PtzOptics` G3 series camera
    PtzOpticsG3,
    /// `PtzOptics` 30X optical zoom camera
    PtzOptics30X,
    /// Sony FR7 camera
    SonyFR7,
    /// Unknown or generic VISCA camera
    Unknown,
}

const PAN_MIN: i16 = -2448;
const PAN_MAX: i16 = 2448;
const TILT_MIN: i16 = -432;
const TILT_MAX: i16 = 1296;
const ZOOM_MIN: u16 = 0x0000;
const ZOOM_MAX_OPTICAL: u16 = 0x4000;
const ZOOM_MAX_SONY_DIGITAL: u16 = 0x7000;
const PAN_SPEED_MAX: u8 = 0x18;
const TILT_SPEED_MAX: u8 = 0x14;
const ZOOM_SPEED_MAX: u8 = 0x07;
const FOCUS_SPEED_MAX: u8 = 0x07;

impl CameraVariant {
    const fn pan_range(self) -> (i16, i16) {
        (PAN_MIN, PAN_MAX)
    }

    const fn tilt_range(self) -> (i16, i16) {
        (TILT_MIN, TILT_MAX)
    }

    const fn zoom_range(self) -> (u16, u16) {
        match self {
            Self::SonyFR7 => (ZOOM_MIN, ZOOM_MAX_SONY_DIGITAL),
            Self::PtzOpticsG2 | Self::PtzOpticsG3 | Self::PtzOptics30X | Self::Unknown => {
                (ZOOM_MIN, ZOOM_MAX_OPTICAL)
            }
        }
    }
}

/// Validate pan position is within camera limits
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the pan position is outside the valid range for the camera model
pub fn validate_pan_position(pos: i16, model: CameraVariant) -> Result<i16, Error> {
    let (min, max) = model.pan_range();
    if pos < min || pos > max {
        Err(Error::ParameterOutOfRange {
            parameter: "pan",
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
/// Returns `Error::ParameterOutOfRange` if the tilt position is outside the valid range for the camera model
pub fn validate_tilt_position(pos: i16, model: CameraVariant) -> Result<i16, Error> {
    let (min, max) = model.tilt_range();
    if pos < min || pos > max {
        Err(Error::ParameterOutOfRange {
            parameter: "tilt",
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
/// Returns `Error::ParameterOutOfRange` if the zoom position is outside the valid range for the camera model
pub fn validate_zoom_position(pos: u16, model: CameraVariant) -> Result<u16, Error> {
    let (min, max) = model.zoom_range();
    if pos < min || pos > max {
        Err(Error::ParameterOutOfRange {
            parameter: "zoom",
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
/// Returns `Error::ParameterOutOfRange` if the pan speed exceeds the maximum allowed value
pub fn validate_pan_speed(speed: u8) -> Result<u8, Error> {
    if speed > PAN_SPEED_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "pan_speed",
            value: i32::from(speed),
            min: 0,
            max: i32::from(PAN_SPEED_MAX),
        })
    } else {
        Ok(speed)
    }
}

/// Validate tilt speed is within limits
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the tilt speed exceeds the maximum allowed value
pub fn validate_tilt_speed(speed: u8) -> Result<u8, Error> {
    if speed > TILT_SPEED_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "tilt_speed",
            value: i32::from(speed),
            min: 0,
            max: i32::from(TILT_SPEED_MAX),
        })
    } else {
        Ok(speed)
    }
}

/// Validate zoom speed is within limits
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the zoom speed exceeds the maximum allowed value
pub fn validate_zoom_speed(speed: u8) -> Result<u8, Error> {
    if speed > ZOOM_SPEED_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "zoom_speed",
            value: i32::from(speed),
            min: 0,
            max: i32::from(ZOOM_SPEED_MAX),
        })
    } else {
        Ok(speed)
    }
}

/// Validate focus speed is within limits
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the focus speed exceeds the maximum allowed value
pub fn validate_focus_speed(speed: u8) -> Result<u8, Error> {
    if speed > FOCUS_SPEED_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "focus_speed",
            value: i32::from(speed),
            min: 0,
            max: i32::from(FOCUS_SPEED_MAX),
        })
    } else {
        Ok(speed)
    }
}

/// Validate iris level is within VISCA range (0x00-0x0C for most cameras)
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the iris level is outside the valid range
pub fn validate_iris_level(_level: u8, _model: CameraVariant) -> Result<u8, Error> {
    // Using VISCA standard range - could be refined per model
    const IRIS_MIN: u8 = 0x00;
    const IRIS_MAX: u8 = 0x0C;

    if _level > IRIS_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "iris_level",
            value: i32::from(_level),
            min: i32::from(IRIS_MIN),
            max: i32::from(IRIS_MAX),
        })
    } else {
        Ok(_level)
    }
}

/// Validate shutter speed is within VISCA range (typically 0x00-0x17)
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the shutter speed is outside the valid range
pub fn validate_shutter_speed(_speed: u16, _model: CameraVariant) -> Result<u16, Error> {
    // Using VISCA standard range - could be refined per model
    const SHUTTER_MIN: u16 = 0x00;
    const SHUTTER_MAX: u16 = 0x17;

    if _speed > SHUTTER_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "shutter_speed",
            value: i32::from(_speed),
            min: i32::from(SHUTTER_MIN),
            max: i32::from(SHUTTER_MAX),
        })
    } else {
        Ok(_speed)
    }
}

/// Validate gain level is within VISCA range (0x00-0x0F for most cameras)
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the gain level is outside the valid range
pub fn validate_gain_level(_level: u8, _model: CameraVariant) -> Result<u8, Error> {
    // Using VISCA standard range - could be refined per model
    const GAIN_MIN: u8 = 0x00;
    const GAIN_MAX: u8 = 0x0F;

    if _level > GAIN_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "gain_level",
            value: i32::from(_level),
            min: i32::from(GAIN_MIN),
            max: i32::from(GAIN_MAX),
        })
    } else {
        Ok(_level)
    }
}

/// Validate brightness level is within VISCA range (0x00-0x11 for most cameras)
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the brightness level is outside the valid range
pub fn validate_brightness_level(_level: u16, _model: CameraVariant) -> Result<u16, Error> {
    // Using VISCA standard range - could be refined per model
    const BRIGHTNESS_MIN: u16 = 0x00;
    const BRIGHTNESS_MAX: u16 = 0x11;

    if _level > BRIGHTNESS_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "brightness_level",
            value: i32::from(_level),
            min: i32::from(BRIGHTNESS_MIN),
            max: i32::from(BRIGHTNESS_MAX),
        })
    } else {
        Ok(_level)
    }
}

/// Validate contrast level is within VISCA range (0x00-0x0E for most cameras)
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the contrast level is outside the valid range
pub fn validate_contrast_level(_level: u8, _model: CameraVariant) -> Result<u8, Error> {
    // Using VISCA standard range - could be refined per model
    const CONTRAST_MIN: u8 = 0x00;
    const CONTRAST_MAX: u8 = 0x0E;

    if _level > CONTRAST_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "contrast_level",
            value: i32::from(_level),
            min: i32::from(CONTRAST_MIN),
            max: i32::from(CONTRAST_MAX),
        })
    } else {
        Ok(_level)
    }
}

/// Validate saturation level is within VISCA range (0x00-0x0E for most cameras)
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the saturation level is outside the valid range
pub fn validate_saturation_level(_level: u8, _model: CameraVariant) -> Result<u8, Error> {
    // Using VISCA standard range - could be refined per model
    const SATURATION_MIN: u8 = 0x00;
    const SATURATION_MAX: u8 = 0x0E;

    if _level > SATURATION_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "saturation_level",
            value: i32::from(_level),
            min: i32::from(SATURATION_MIN),
            max: i32::from(SATURATION_MAX),
        })
    } else {
        Ok(_level)
    }
}

/// Validate sharpness level using model-specific VISCA ranges.
///
/// PTZOptics cameras support the extended `0x00..=0x0F` range verified from
/// hardware validation, while the generic VISCA range remains `0x00..=0x0B`.
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the sharpness level is outside the valid range
pub fn validate_sharpness_level(level: u8, model: CameraVariant) -> Result<u8, Error> {
    const SHARPNESS_MIN: u8 = 0x00;

    let sharpness_max = match model {
        CameraVariant::PtzOpticsG2 | CameraVariant::PtzOpticsG3 | CameraVariant::PtzOptics30X => {
            0x0F
        }
        CameraVariant::SonyFR7 | CameraVariant::Unknown => 0x0B,
    };

    if level > sharpness_max {
        Err(Error::ParameterOutOfRange {
            parameter: "sharpness_level",
            value: i32::from(level),
            min: i32::from(SHARPNESS_MIN),
            max: i32::from(sharpness_max),
        })
    } else {
        Ok(level)
    }
}

/// Validate hue level is within VISCA range (0x00-0x0E for most cameras)
///
/// # Errors
///
/// Returns `Error::ParameterOutOfRange` if the hue level is outside the valid range
pub fn validate_hue_level(_level: u8, _model: CameraVariant) -> Result<u8, Error> {
    // Using VISCA standard range - could be refined per model
    const HUE_MIN: u8 = 0x00;
    const HUE_MAX: u8 = 0x0E;

    if _level > HUE_MAX {
        Err(Error::ParameterOutOfRange {
            parameter: "hue_level",
            value: i32::from(_level),
            min: i32::from(HUE_MIN),
            max: i32::from(HUE_MAX),
        })
    } else {
        Ok(_level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_functions() {
        assert!(validate_pan_position(0, CameraVariant::PtzOpticsG2).is_ok());
        assert!(validate_pan_position(5000, CameraVariant::PtzOpticsG2).is_err());

        assert!(validate_tilt_position(0, CameraVariant::PtzOpticsG2).is_ok());
        assert!(validate_tilt_position(-1000, CameraVariant::PtzOpticsG2).is_err());

        assert!(validate_pan_speed(12).is_ok());
        assert!(validate_pan_speed(30).is_err());
    }
}
