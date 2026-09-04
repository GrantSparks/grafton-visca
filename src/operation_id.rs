//! Opaque identity shared by the mode-native operation handle façades.

use std::{fmt, num::NonZeroU64};

/// Opaque identity for an admitted operation.
///
/// An operation ID is useful for logging and diagnostics only. It does not
/// authorize lifecycle mutation; waiting and cancellation require ownership of
/// the corresponding operation handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn from_raw(value: u64) -> Self {
        Self(NonZeroU64::new(value).unwrap_or(NonZeroU64::MIN))
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
