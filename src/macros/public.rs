//! Feature selection for the checked value attributes.
//!
//! Selection happens in the library rather than its caller, so generated
//! serde, schema and TypeScript support follows this library's feature set.

/// Builds a const-safe invalid-value error for a finite scalar domain.
///
/// This support function lets a generated constructor remain const without
/// allocating diagnostic text. Wire decoding applies the same domain checks.
#[doc(hidden)]
#[must_use]
pub const fn invalid_value(parameter: &'static str, reason: &'static str) -> crate::Error {
    crate::Error::InvalidParameter {
        parameter,
        value: ::std::borrow::Cow::Borrowed("outside the declared domain"),
        reason: ::std::borrow::Cow::Borrowed(reason),
    }
}

#[cfg(not(feature = "serde"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_value_helpers {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__value! {
            crate = $crate; []; $($declaration)*
        }
    };
}

#[cfg(all(feature = "serde", not(feature = "schemars"), not(feature = "ts-rs")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_value_helpers {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__value! {
            crate = $crate; [serde]; $($declaration)*
        }
    };
}

#[cfg(all(feature = "schemars", not(feature = "ts-rs")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_value_helpers {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__value! {
            crate = $crate; [serde, schemars]; $($declaration)*
        }
    };
}

#[cfg(all(feature = "ts-rs", not(feature = "schemars")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_value_helpers {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__value! {
            crate = $crate; [serde, ts_rs]; $($declaration)*
        }
    };
}

#[cfg(all(feature = "schemars", feature = "ts-rs"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_value_helpers {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__value! {
            crate = $crate; [serde, schemars, ts_rs]; $($declaration)*
        }
    };
}
