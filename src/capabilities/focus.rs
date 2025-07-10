//! Focus capability trait and associated types.

use crate::capabilities::ValidationError;

/// Trait for cameras that support focus control.
///
/// This trait defines the constants and capabilities for focus operations.
/// Cameras implementing this trait gain access to focus control methods.
pub trait SupportsFocus {
    /// Minimum focus position (near limit) in VISCA units.
    const FOCUS_NEAR_LIMIT: u16;
    
    /// Maximum focus position (far limit) in VISCA units.  
    const FOCUS_FAR_LIMIT: u16;
    
    /// Whether camera supports auto focus mode.
    const SUPPORTS_AUTO_FOCUS: bool;
    
    /// Whether camera supports one-push auto focus.
    /// This triggers a single auto focus operation then returns to manual.
    const SUPPORTS_ONE_PUSH_FOCUS: bool;
    
    /// Whether camera supports focus zone selection.
    /// Allows selecting which part of image to focus on.
    const SUPPORTS_FOCUS_ZONE: bool = false;
    
    /// Maximum focus speed for manual focus operations.
    /// Usually 0-7 where 0 is slowest, 7 is fastest.
    const MAX_FOCUS_SPEED: u8 = 7;
    
    /// Whether camera supports auto focus sensitivity adjustment.
    const SUPPORTS_AF_SENSITIVITY: bool = false;
}

/// Extension trait that adds validation methods to cameras with focus support.
pub trait FocusExt: SupportsFocus {
    /// Validate a focus position is within range.
    fn validate_focus_position(&self, position: u16) -> Result<u16, ValidationError> {
        if position >= Self::FOCUS_NEAR_LIMIT && position <= Self::FOCUS_FAR_LIMIT {
            Ok(position)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "focus position",
                value: position as f64,
                min: Self::FOCUS_NEAR_LIMIT as f64,
                max: Self::FOCUS_FAR_LIMIT as f64,
            })
        }
    }
    
    /// Validate and clamp focus speed.
    fn validate_focus_speed(&self, speed: u8) -> u8 {
        speed.min(Self::MAX_FOCUS_SPEED)
    }
    
    /// Convert normalized focus (0.0=near, 1.0=far) to VISCA units.
    fn normalized_to_focus_units(&self, normalized: f32) -> Result<u16, ValidationError> {
        if !(0.0..=1.0).contains(&normalized) {
            return Err(ValidationError::InvalidValue {
                parameter: "normalized focus",
                message: "Must be between 0.0 and 1.0".to_string(),
            });
        }
        
        let range = Self::FOCUS_FAR_LIMIT - Self::FOCUS_NEAR_LIMIT;
        let position = Self::FOCUS_NEAR_LIMIT + (normalized * range as f32) as u16;
        Ok(position)
    }
    
    /// Convert VISCA units to normalized focus (0.0=near, 1.0=far).
    fn focus_units_to_normalized(&self, units: u16) -> f32 {
        let range = Self::FOCUS_FAR_LIMIT - Self::FOCUS_NEAR_LIMIT;
        (units - Self::FOCUS_NEAR_LIMIT) as f32 / range as f32
    }
    
    /// Check if auto focus is available.
    fn can_auto_focus(&self) -> bool {
        Self::SUPPORTS_AUTO_FOCUS
    }
    
    /// Check if one-push focus is available.
    fn can_one_push_focus(&self) -> bool {
        Self::SUPPORTS_ONE_PUSH_FOCUS
    }
}

// Automatic implementation for all types that support focus
impl<T: SupportsFocus> FocusExt for T {}

/// Focus zone selection for cameras that support it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusZone {
    /// Center of the image.
    Center,
    /// Top of the image.
    Top,
    /// Bottom of the image. 
    Bottom,
    /// Custom zone with coordinates.
    Custom { 
        /// X coordinate (0-15).
        x: u8, 
        /// Y coordinate (0-15).
        y: u8 
    },
}

/// Auto focus sensitivity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoFocusSensitivity {
    /// Normal sensitivity.
    Normal,
    /// Low sensitivity - less reactive to changes.
    Low,
    /// High sensitivity - more reactive to changes.
    High,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestCamera;
    
    impl SupportsFocus for TestCamera {
        const FOCUS_NEAR_LIMIT: u16 = 0x1000;
        const FOCUS_FAR_LIMIT: u16 = 0xF000;
        const SUPPORTS_AUTO_FOCUS: bool = true;
        const SUPPORTS_ONE_PUSH_FOCUS: bool = true;
    }
    
    #[test]
    fn test_focus_validation() {
        let camera = TestCamera;
        
        assert!(camera.validate_focus_position(0x1000).is_ok());
        assert!(camera.validate_focus_position(0xF000).is_ok());
        assert!(camera.validate_focus_position(0x8000).is_ok());
        assert!(camera.validate_focus_position(0x0FFF).is_err());
        assert!(camera.validate_focus_position(0xF001).is_err());
    }
    
    #[test]
    fn test_normalized_conversion() {
        let camera = TestCamera;
        
        assert_eq!(camera.normalized_to_focus_units(0.0).unwrap(), 0x1000);
        assert_eq!(camera.normalized_to_focus_units(1.0).unwrap(), 0xF000);
        
        // Test round trip
        let pos = camera.normalized_to_focus_units(0.5).unwrap();
        let normalized = camera.focus_units_to_normalized(pos);
        assert!((normalized - 0.5).abs() < 0.01);
    }
}