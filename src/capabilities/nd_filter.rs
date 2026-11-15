//! Source-backed variable Neutral Density (ND) filter metadata.

use crate::{capabilities::ValidationError, command::NdFilterValue};

/// Runtime ND metadata, independent of typed control permission.
///
/// Implement [`crate::capabilities::HasNdFilter`] only when the profile has
/// evidence for the typed variable-density command family.
pub trait NdFilterMetadata {
    /// Source-backed ND mode; defaults to no ND filter.
    const ND_MODE: NdFilterMode = NdFilterMode::None;
}

/// Validation and discovery helpers for ND metadata.
pub trait NdFilterMetadataExt: NdFilterMetadata {
    /// Validates the same variable-density code as the typed request path.
    fn validate_nd_filter(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::ND_MODE {
            NdFilterMode::None => Err(ValidationError::NotSupported("ND filter")),
            NdFilterMode::Variable => {
                NdFilterValue::new(u16::from(value))
                    .map(|_| value)
                    .map_err(|_| {
                        ValidationError::out_of_range(
                            "ND filter value",
                            f64::from(value),
                            0.0,
                            f64::from(NdFilterValue::MAX_VALUE),
                        )
                    })
            }
        }
    }

    /// Describes the profile's ND mode.
    fn nd_filter_description(&self) -> &'static str {
        match Self::ND_MODE {
            NdFilterMode::None => "No ND filter",
            NdFilterMode::Variable => "Variable ND filter",
        }
    }

    /// Returns whether the profile declares a variable ND filter.
    fn has_nd_filter(&self) -> bool {
        matches!(Self::ND_MODE, NdFilterMode::Variable)
    }
}
impl<T: NdFilterMetadata> NdFilterMetadataExt for T {}

/// ND modes with a sourced typed implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum NdFilterMode {
    /// No ND filter.
    None,
    /// Variable density, encoded as `00..=14` hexadecimal.
    Variable,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    struct NoNd;
    impl NdFilterMetadata for NoNd {}
    struct VariableNd;
    impl NdFilterMetadata for VariableNd {
        const ND_MODE: NdFilterMode = NdFilterMode::Variable;
    }

    #[test]
    fn absent_nd_has_no_accepted_values() {
        assert!(!NoNd.has_nd_filter());
        assert_eq!(NoNd.nd_filter_description(), "No ND filter");
        for value in 0..=u8::MAX {
            assert_eq!(
                NoNd.validate_nd_filter(value),
                Err(ValidationError::NotSupported("ND filter"))
            );
        }
    }

    #[test]
    fn variable_nd_helper_and_typed_value_agree_for_every_byte() {
        assert!(VariableNd.has_nd_filter());
        assert_eq!(VariableNd.nd_filter_description(), "Variable ND filter");
        for value in 0..=u8::MAX {
            assert_eq!(
                VariableNd.validate_nd_filter(value).is_ok(),
                NdFilterValue::new(u16::from(value)).is_ok(),
                "density code {value}"
            );
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn only_implemented_nd_modes_round_trip() {
        for mode in [NdFilterMode::None, NdFilterMode::Variable] {
            let json = serde_json::to_string(&mode).expect("serialize ND mode");
            assert_eq!(
                serde_json::from_str::<NdFilterMode>(&json).expect("ND mode"),
                mode
            );
        }
        for old in [r#"{"Stepped":3}"#, r#"{"Fixed":3}"#] {
            assert!(serde_json::from_str::<NdFilterMode>(old).is_err());
        }
    }
}
