//! Validation error types for capability traits.

use std::fmt;

/// Errors that can occur during parameter validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Parameter is out of the valid range.
    OutOfRange {
        /// Name of the parameter.
        parameter: &'static str,
        /// Actual value provided.
        value: f64,
        /// Minimum valid value.
        min: f64,
        /// Maximum valid value.
        max: f64,
    },

    /// Parameter has an invalid value.
    InvalidValue {
        /// Name of the parameter.
        parameter: &'static str,
        /// Description of why the value is invalid.
        message: String,
    },

    /// Feature is not supported by this camera.
    NotSupported(&'static str),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::OutOfRange {
                parameter,
                value,
                min,
                max,
            } => {
                write!(
                    f,
                    "{} value {} is out of range [{}, {}]",
                    parameter, value, min, max
                )
            }
            ValidationError::InvalidValue { parameter, message } => {
                write!(f, "Invalid {} value: {}", parameter, message)
            }
            ValidationError::NotSupported(feature) => {
                write!(f, "{} is not supported by this camera", feature)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_out_of_range_display() {
        let err = ValidationError::OutOfRange {
            parameter: "zoom",
            value: 150.0,
            min: 0.0,
            max: 100.0,
        };

        assert_eq!(err.to_string(), "zoom value 150 is out of range [0, 100]");
    }

    #[test]
    fn test_invalid_value_display() {
        let err = ValidationError::InvalidValue {
            parameter: "shutter speed",
            message: "Must be one of the predefined values".to_string(),
        };

        assert_eq!(
            err.to_string(),
            "Invalid shutter speed value: Must be one of the predefined values"
        );
    }

    #[test]
    fn test_not_supported_display() {
        let err = ValidationError::NotSupported("ND filter");

        assert_eq!(err.to_string(), "ND filter is not supported by this camera");
    }
}
