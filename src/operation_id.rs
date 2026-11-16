//! Opaque identity shared by the mode-native operation handle façades.

use std::{fmt, num::NonZeroU64};

/// Opaque identity for an admitted operation.
///
/// An operation ID is useful for logging and diagnostics only. It does not
/// authorize lifecycle mutation; waiting and cancellation require ownership of
/// the corresponding operation handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct OperationId(NonZeroU64);

impl OperationId {
    /// Returns the numeric identity of this operation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    /// Returns this identity as its non-zero representation.
    #[must_use]
    pub const fn as_nonzero(self) -> NonZeroU64 {
        self.0
    }

    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) const fn from_nonzero(value: NonZeroU64) -> Self {
        Self(value)
    }

    #[cfg(test)]
    #[track_caller]
    #[allow(clippy::expect_used)]
    pub(crate) fn from_raw(value: u64) -> Self {
        Self::from_nonzero(
            NonZeroU64::new(value).expect("test operation identity must be non-zero"),
        )
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(all(test, feature = "serde"))]
mod portable_data_tests {
    use super::*;

    #[test]
    fn persisted_operation_identity_keeps_its_nonzero_domain() -> Result<(), serde_json::Error> {
        for value in [1, 7, u64::MAX] {
            let id = OperationId::from_raw(value);
            let json = serde_json::to_value(id)?;
            assert_eq!(json, serde_json::json!(value));
            let restored: OperationId = serde_json::from_value(json)?;
            assert_eq!(restored, id);
            assert_eq!(restored.as_nonzero().get(), value);
        }
        for invalid in ["0", "-1", "null", "\"7\"", "{\"id\":7}"] {
            assert!(serde_json::from_str::<OperationId>(invalid).is_err());
        }
        Ok(())
    }

    #[test]
    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    fn identity_schema_and_typescript_describe_the_portable_number() {
        let schema = schemars::schema_for!(OperationId);
        assert_eq!(schema.as_value()["type"], "integer");
        assert_eq!(schema.as_value()["minimum"], 1);
        assert_eq!(
            <OperationId as ts_rs::TS>::decl(&ts_rs::Config::default()),
            "type OperationId = bigint;"
        );
    }
}
