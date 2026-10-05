//! Public API macros for extending the grafton-visca library.
//!
//! This module contains macros that are part of the stable public API and are intended
//! for use by library consumers to extend functionality.

/// Create a type with range validation.
///
/// This macro generates a newtype wrapper that enforces value constraints
/// at the type level. It's useful for creating custom parameter types that
/// work seamlessly with the VISCA command system.
///
/// The generated types automatically include `serde`, `schemars`, and `ts-rs`
/// support when the respective `grafton-visca` features are enabled.
///
/// # Example
/// ```
/// use grafton_visca::visca_range_type;
///
/// visca_range_type! {
///     /// Zoom speed level from 0 (slow) to 7 (fast)
///     ZoomSpeed: u8 {
///         min: 0,
///         max: 7
///     }
/// }
///
/// # fn main() -> Result<(), grafton_visca::Error> {
/// // Creating a valid speed
/// let speed = ZoomSpeed::new(5)?;
/// assert_eq!(speed.value(), 5);
///
/// // Attempting to create an invalid speed
/// let result = ZoomSpeed::new(10);
/// assert!(result.is_err());
/// # Ok(())
/// # }
/// ```
///
/// # Generated API
///
/// The macro expands through the same generator as `#[derive(ViscaValue)]`,
/// so both have the same constructor, bounds, accessor and error contract:
/// - `MIN: Self` and `MAX: Self` - The inclusive bounds; `min > max` fails to
///   compile
/// - `const fn new(value: T) -> Result<Self, Error>` - Creates a new instance
///   with validation; a value outside `MIN..=MAX` is
///   [`Error::ParameterOutOfRange`](crate::Error::ParameterOutOfRange)
/// - `const fn value(self) -> T` - Returns the inner value
///
/// `ParameterOutOfRange` reports its bounds as `i32`, so `T` must convert into
/// `i32` losslessly: `u8`, `u16`, `i8`, `i16` or `i32`.
///
/// It also implements:
/// - `TryFrom<T>` for convenient conversions
/// - `From<YourType> for T` to extract the inner value
/// - Common derives: `Debug`, `Copy`, `Clone`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`
/// - When enabled: `serde::Serialize`, `serde::Deserialize`, `schemars::JsonSchema`, `ts_rs::TS`
#[macro_export]
macro_rules! visca_range_type {
    (
        $(#[$meta:meta])*
        $name:ident : $inner:ty {
            min: $min:expr,
            max: $max:expr
        }
    ) => {
        $crate::__grafton_visca_newtype! {
            $(#[$meta])*
            $name : $inner {
                min: $min,
                max: $max
            }
        }
    };
}

// The declaration adapter needs the helper derives this crate was built with.
// `cfg` inside a `macro_rules!` body is evaluated in the calling crate, so the
// selection happens here, once per helper-feature combination; the adapter
// owns the declaration grammar. This crate's own checked value types invoke
// the selector directly, so the helper derives are written in one place.

#[cfg(not(feature = "serde"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_newtype {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__grafton_visca_newtype! {
            crate = $crate;
            []
            $($declaration)*
        }
    };
}

#[cfg(all(feature = "serde", not(feature = "schemars"), not(feature = "ts-rs")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_newtype {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__grafton_visca_newtype! {
            crate = $crate;
            [serde]
            $($declaration)*
        }
    };
}

#[cfg(all(feature = "schemars", not(feature = "ts-rs")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_newtype {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__grafton_visca_newtype! {
            crate = $crate;
            [serde, schemars]
            $($declaration)*
        }
    };
}

#[cfg(all(feature = "ts-rs", not(feature = "schemars")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_newtype {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__grafton_visca_newtype! {
            crate = $crate;
            [serde, ts_rs]
            $($declaration)*
        }
    };
}

#[cfg(all(feature = "schemars", feature = "ts-rs"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_newtype {
    ($($declaration:tt)*) => {
        $crate::__macro_support::__grafton_visca_newtype! {
            crate = $crate;
            [serde, schemars, ts_rs]
            $($declaration)*
        }
    };
}

/// Inner types a range-checked newtype may wrap.
///
/// `Error::ParameterOutOfRange` reports its value and bounds as `i32`, so a
/// range's inner type must convert into `i32` without loss. The trait is
/// sealed to exactly those primitives, which lets generated code widen with a
/// lossless `as` cast inside `const fn new`.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be the inner type of a range-checked VISCA newtype",
    label = "range bounds are reported as `i32`",
    note = "use `u8`, `u16`, `i8`, `i16` or `i32` as the inner type"
)]
pub trait RangeInner: range_inner::Sealed {}

mod range_inner {
    pub trait Sealed {}
}

macro_rules! impl_range_inner {
    ($($inner:ty),*) => {
        $(
            impl range_inner::Sealed for $inner {}
            impl RangeInner for $inner {}
        )*
    };
}

impl_range_inner!(u8, u16, i8, i16, i32);
