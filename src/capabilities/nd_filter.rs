//! Neutral Density (ND) filter capability trait and associated types.

use crate::capabilities::ValidationError;

/// Trait for cameras that support ND filter control.
///
/// This trait defines the constants and capabilities for ND filter operations.
/// ND filters reduce the amount of light entering the camera without affecting color.
pub trait SupportsNDFilter {
    /// ND filter mode determines how the filter operates.
    const ND_MODE: NDFilterMode;
    
    /// Number of discrete ND filter steps (for stepped filters).
    /// None for continuous/variable filters.
    const ND_STEPS: Option<u8>;
}

/// Extension trait that adds validation methods to cameras with ND filter support.
pub trait NDFilterExt: SupportsNDFilter {
    /// Validate an ND filter setting based on the camera's ND mode.
    fn validate_nd_filter(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::ND_MODE {
            NDFilterMode::None => {
                Err(ValidationError::NotSupported("ND filter"))
            }
            NDFilterMode::Fixed(fixed_value) => {
                // Only 0 (off) or the fixed value are valid
                if value == 0 || value == fixed_value {
                    Ok(value)
                } else {
                    Err(ValidationError::InvalidValue {
                        parameter: "ND filter",
                        message: format!("Only 0 (off) or {} (on) are valid", fixed_value),
                    })
                }
            }
            NDFilterMode::Stepped(steps) => {
                if value <= steps {
                    Ok(value)
                } else {
                    Err(ValidationError::OutOfRange {
                        parameter: "ND filter step",
                        value: value as f64,
                        min: 0.0,
                        max: steps as f64,
                    })
                }
            }
            NDFilterMode::Variable => {
                // For variable ND, typically 0-255 range
                Ok(value)
            }
        }
    }
    
    /// Get a human-readable description of the ND filter mode.
    fn nd_filter_description(&self) -> &'static str {
        match Self::ND_MODE {
            NDFilterMode::None => "No ND filter",
            NDFilterMode::Fixed(_) => "Fixed ND filter",
            NDFilterMode::Stepped(_) => "Stepped ND filter",
            NDFilterMode::Variable => "Variable ND filter",
        }
    }
    
    /// Check if ND filter is available.
    fn has_nd_filter(&self) -> bool {
        !matches!(Self::ND_MODE, NDFilterMode::None)
    }
    
    /// Get the number of ND steps if applicable.
    fn nd_step_count(&self) -> Option<u8> {
        match Self::ND_MODE {
            NDFilterMode::Stepped(steps) => Some(steps),
            _ => Self::ND_STEPS,
        }
    }
}

// Automatic implementation for all types that support ND filter
impl<T: SupportsNDFilter> NDFilterExt for T {}

/// ND filter operating modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDFilterMode {
    /// No ND filter available.
    None,
    /// Fixed ND filter with a single density value.
    /// Can only be on (at the fixed value) or off.
    Fixed(u8),
    /// Stepped ND filter with discrete positions.
    /// Value indicates number of steps (e.g., 3 for 1/4, 1/16, 1/64).
    Stepped(u8),
    /// Continuously variable ND filter.
    /// Can be adjusted to any value in the range.
    Variable,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct NoNDCamera;
    impl SupportsNDFilter for NoNDCamera {
        const ND_MODE: NDFilterMode = NDFilterMode::None;
        const ND_STEPS: Option<u8> = None;
    }
    
    struct FixedNDCamera;
    impl SupportsNDFilter for FixedNDCamera {
        const ND_MODE: NDFilterMode = NDFilterMode::Fixed(3);
        const ND_STEPS: Option<u8> = None;
    }
    
    struct SteppedNDCamera;
    impl SupportsNDFilter for SteppedNDCamera {
        const ND_MODE: NDFilterMode = NDFilterMode::Stepped(3);
        const ND_STEPS: Option<u8> = Some(3);
    }
    
    struct VariableNDCamera;
    impl SupportsNDFilter for VariableNDCamera {
        const ND_MODE: NDFilterMode = NDFilterMode::Variable;
        const ND_STEPS: Option<u8> = None;
    }
    
    #[test]
    fn test_no_nd_filter() {
        let camera = NoNDCamera;
        
        assert!(!camera.has_nd_filter());
        assert!(camera.validate_nd_filter(0).is_err());
        assert_eq!(camera.nd_filter_description(), "No ND filter");
    }
    
    #[test]
    fn test_fixed_nd_filter() {
        let camera = FixedNDCamera;
        
        assert!(camera.has_nd_filter());
        assert!(camera.validate_nd_filter(0).is_ok()); // Off
        assert!(camera.validate_nd_filter(3).is_ok()); // On at fixed value
        assert!(camera.validate_nd_filter(1).is_err()); // Invalid
        assert!(camera.validate_nd_filter(2).is_err()); // Invalid
    }
    
    #[test]
    fn test_stepped_nd_filter() {
        let camera = SteppedNDCamera;
        
        assert!(camera.has_nd_filter());
        assert!(camera.validate_nd_filter(0).is_ok()); // Off
        assert!(camera.validate_nd_filter(1).is_ok()); // Step 1
        assert!(camera.validate_nd_filter(2).is_ok()); // Step 2
        assert!(camera.validate_nd_filter(3).is_ok()); // Step 3
        assert!(camera.validate_nd_filter(4).is_err()); // Out of range
        assert_eq!(camera.nd_step_count(), Some(3));
    }
    
    #[test]
    fn test_variable_nd_filter() {
        let camera = VariableNDCamera;
        
        assert!(camera.has_nd_filter());
        assert!(camera.validate_nd_filter(0).is_ok());
        assert!(camera.validate_nd_filter(128).is_ok());
        assert!(camera.validate_nd_filter(255).is_ok());
    }
}