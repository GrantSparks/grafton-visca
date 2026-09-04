//! Neutral Density (ND) filter metadata trait and associated types.

use std::borrow::Cow;

use crate::capabilities::ValidationError;

/// Metadata for a camera profile's ND filter capability.
///
/// This trait supplies runtime discovery defaults. It does not mean the typed
/// ND filter control API is available for a profile; use
/// [`crate::capabilities::HasNdFilter`] for that compile-time support marker.
pub trait NdFilterMetadata {
    /// ND filter mode determines how the filter operates.
    ///
    /// Default: [`NdFilterMode::None`] — no ND filter available.
    const ND_MODE: NdFilterMode = NdFilterMode::None;

    /// Number of discrete ND filter steps (for stepped filters).
    /// None for continuous/variable filters.
    ///
    /// Default: `None`.
    const ND_STEPS: Option<u8> = None;
}

/// Extension trait that adds validation methods to ND filter metadata.
pub trait NdFilterMetadataExt: NdFilterMetadata {
    /// Validate an ND filter setting based on the camera's ND mode.
    fn validate_nd_filter(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::ND_MODE {
            NdFilterMode::None => Err(ValidationError::NotSupported("ND filter")),
            NdFilterMode::Fixed(fixed_value) => {
                // Only 0 (off) or the fixed value are valid
                if value == 0 || value == fixed_value {
                    Ok(value)
                } else {
                    Err(ValidationError::InvalidValue {
                        parameter: "ND filter",
                        message: Cow::Owned(format!(
                            "Only 0 (off) or {fixed_value} (on) are valid"
                        )),
                    })
                }
            }
            NdFilterMode::Stepped(steps) => {
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
            NdFilterMode::Variable => {
                // For variable ND, typically 0-255 range
                Ok(value)
            }
        }
    }

    /// Get a human-readable description of the ND filter mode.
    fn nd_filter_description(&self) -> &'static str {
        match Self::ND_MODE {
            NdFilterMode::None => "No ND filter",
            NdFilterMode::Fixed(_) => "Fixed ND filter",
            NdFilterMode::Stepped(_) => "Stepped ND filter",
            NdFilterMode::Variable => "Variable ND filter",
        }
    }

    /// Check if ND filter is available.
    fn has_nd_filter(&self) -> bool {
        !matches!(Self::ND_MODE, NdFilterMode::None)
    }

    /// Get the number of ND steps if applicable.
    fn nd_step_count(&self) -> Option<u8> {
        match Self::ND_MODE {
            NdFilterMode::Stepped(steps) => Some(steps),
            _ => Self::ND_STEPS,
        }
    }
}

impl<T: NdFilterMetadata> NdFilterMetadataExt for T {}

/// ND filter operating modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum NdFilterMode {
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

    struct NoNdCamera;
    impl NdFilterMetadata for NoNdCamera {
        const ND_MODE: NdFilterMode = NdFilterMode::None;
        const ND_STEPS: Option<u8> = None;
    }

    struct FixedNdCamera;
    impl NdFilterMetadata for FixedNdCamera {
        const ND_MODE: NdFilterMode = NdFilterMode::Fixed(3);
        const ND_STEPS: Option<u8> = None;
    }

    struct SteppedNdCamera;
    impl NdFilterMetadata for SteppedNdCamera {
        const ND_MODE: NdFilterMode = NdFilterMode::Stepped(3);
        const ND_STEPS: Option<u8> = Some(3);
    }

    struct VariableNdCamera;
    impl NdFilterMetadata for VariableNdCamera {
        const ND_MODE: NdFilterMode = NdFilterMode::Variable;
        const ND_STEPS: Option<u8> = None;
    }

    #[test]
    fn test_no_nd_filter() {
        let camera = NoNdCamera;

        assert!(!camera.has_nd_filter());
        assert!(camera.validate_nd_filter(0).is_err());
        assert_eq!(camera.nd_filter_description(), "No ND filter");
    }

    #[test]
    fn test_fixed_nd_filter() {
        let camera = FixedNdCamera;

        assert!(camera.has_nd_filter());
        assert!(camera.validate_nd_filter(0).is_ok()); // Off
        assert!(camera.validate_nd_filter(3).is_ok()); // On at fixed value
        assert!(camera.validate_nd_filter(1).is_err()); // Invalid
        assert!(camera.validate_nd_filter(2).is_err()); // Invalid
    }

    #[test]
    fn test_stepped_nd_filter() {
        let camera = SteppedNdCamera;

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
        let camera = VariableNdCamera;

        assert!(camera.has_nd_filter());
        assert!(camera.validate_nd_filter(0).is_ok());
        assert!(camera.validate_nd_filter(128).is_ok());
        assert!(camera.validate_nd_filter(255).is_ok());
    }
}
