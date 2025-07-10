//! Zoom capability trait and associated types.

use std::ops::Range;

use crate::capabilities::ValidationError;

/// Trait for cameras that support zoom operations.
///
/// This trait defines the constants and capabilities for zoom control.
/// Cameras implementing this trait gain access to zoom methods.
pub trait SupportsZoom {
    /// Maximum optical zoom position in VISCA units.
    /// For PTZOptics G2 this is 0x4000 (20x optical zoom).
    const OPTICAL_ZOOM_MAX: u16;
    
    /// Maximum digital zoom position if supported.
    /// None if camera doesn't support digital zoom.
    const DIGITAL_ZOOM_MAX: Option<u16>;
    
    /// Valid range for variable zoom speed.
    /// Usually 0-7 where 0 is slowest, 7 is fastest.
    const ZOOM_SPEED_RANGE: Range<u8>;
    
    /// Whether camera supports direct zoom positioning.
    /// If false, zoom must be achieved through zoom in/out commands.
    const SUPPORTS_DIRECT_ZOOM: bool = true;
    
    /// Whether camera supports variable speed zoom.
    /// If false, only standard speed zoom is available.
    const SUPPORTS_VARIABLE_ZOOM: bool = true;
    
    /// Conversion factor from magnification (e.g., 20x) to VISCA units.
    const ZOOM_MAGNIFICATION_TO_UNITS: f32;
}

/// Extension trait that adds validation methods to cameras with zoom support.
pub trait ZoomExt: SupportsZoom {
    /// Validate a zoom position is within range.
    fn validate_zoom_position(&self, position: u16) -> Result<u16, ValidationError> {
        let max = Self::DIGITAL_ZOOM_MAX.unwrap_or(Self::OPTICAL_ZOOM_MAX);
        
        if position <= max {
            Ok(position)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "zoom position",
                value: position as f64,
                min: 0.0,
                max: max as f64,
            })
        }
    }
    
    /// Validate and clamp zoom speed to valid range.
    fn validate_zoom_speed(&self, speed: u8) -> u8 {
        speed.clamp(Self::ZOOM_SPEED_RANGE.start, Self::ZOOM_SPEED_RANGE.end - 1)
    }
    
    /// Check if position is in digital zoom range.
    fn is_digital_zoom(&self, position: u16) -> bool {
        position > Self::OPTICAL_ZOOM_MAX
    }
    
    /// Convert normalized zoom (0.0-1.0) to VISCA units.
    fn normalized_to_zoom_units(&self, normalized: f32) -> Result<u16, ValidationError> {
        if !(0.0..=1.0).contains(&normalized) {
            return Err(ValidationError::InvalidValue {
                parameter: "normalized zoom",
                message: "Must be between 0.0 and 1.0".to_string(),
            });
        }
        
        let max = Self::DIGITAL_ZOOM_MAX.unwrap_or(Self::OPTICAL_ZOOM_MAX);
        Ok((normalized * max as f32) as u16)
    }
    
    /// Convert VISCA units to normalized zoom (0.0-1.0).
    fn zoom_units_to_normalized(&self, units: u16) -> f32 {
        let max = Self::DIGITAL_ZOOM_MAX.unwrap_or(Self::OPTICAL_ZOOM_MAX);
        (units as f32) / (max as f32)
    }
    
    /// Convert magnification (e.g., 5.0 for 5x) to VISCA units.
    fn magnification_to_zoom_units(&self, magnification: f32) -> Result<u16, ValidationError> {
        if magnification < 1.0 {
            return Err(ValidationError::InvalidValue {
                parameter: "zoom magnification",
                message: "Must be at least 1.0x".to_string(),
            });
        }
        
        let units = ((magnification - 1.0) * Self::ZOOM_MAGNIFICATION_TO_UNITS) as u16;
        self.validate_zoom_position(units)
    }
    
    /// Convert VISCA units to magnification (e.g., 5.0 for 5x).
    fn zoom_units_to_magnification(&self, units: u16) -> f32 {
        1.0 + (units as f32 / Self::ZOOM_MAGNIFICATION_TO_UNITS)
    }
}

// Automatic implementation for all types that support zoom
impl<T: SupportsZoom> ZoomExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestCamera;
    
    impl SupportsZoom for TestCamera {
        const OPTICAL_ZOOM_MAX: u16 = 0x4000;
        const DIGITAL_ZOOM_MAX: Option<u16> = Some(0x7000);
        const ZOOM_SPEED_RANGE: Range<u8> = 0..8;
        const ZOOM_MAGNIFICATION_TO_UNITS: f32 = 862.3; // (0x4000 - 1) / 19 for 20x zoom
    }
    
    #[test]
    fn test_zoom_validation() {
        let camera = TestCamera;
        
        assert!(camera.validate_zoom_position(0x4000).is_ok());
        assert!(camera.validate_zoom_position(0x7000).is_ok());
        assert!(camera.validate_zoom_position(0x8000).is_err());
    }
    
    #[test]
    fn test_digital_zoom_detection() {
        let camera = TestCamera;
        
        assert!(!camera.is_digital_zoom(0x4000));
        assert!(camera.is_digital_zoom(0x4001));
        assert!(camera.is_digital_zoom(0x7000));
    }
    
    #[test]
    fn test_normalized_conversion() {
        let camera = TestCamera;
        
        assert_eq!(camera.normalized_to_zoom_units(0.0).unwrap(), 0);
        assert_eq!(camera.normalized_to_zoom_units(1.0).unwrap(), 0x7000);
        assert!(camera.normalized_to_zoom_units(1.1).is_err());
    }
    
    #[test]
    fn test_magnification_conversion() {
        let camera = TestCamera;
        
        // 1x = minimum zoom
        assert_eq!(camera.magnification_to_zoom_units(1.0).unwrap(), 0);
        
        // 20x = full optical zoom (0x4000)
        let units = camera.magnification_to_zoom_units(20.0).unwrap();
        assert!((units as i32 - 0x3FFF).abs() <= 20); // Allow small rounding error
    }
}