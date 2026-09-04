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
/// The macro generates a struct with the following methods:
/// - `new(value: T) -> Result<Self, Error>` - Creates a new instance with validation
/// - `value(&self) -> T` - Returns the inner value
/// - `MIN: T` - The minimum allowed value
/// - `MAX: T` - The maximum allowed value
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
        $crate::__grafton_visca_range_type_decl! {
            $(#[$meta])*
            $name : $inner {
                min: $min,
                max: $max
            }
        }
    };
}

#[cfg(not(feature = "serde"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_range_type_decl {
    ($(#[$meta:meta])* $name:ident : $inner:ty { min: $min:expr, max: $max:expr }) => {
        $crate::__macro_support::__grafton_visca_range_type_decl! {
            crate = $crate;
            []
            $(#[$meta])*
            $name : $inner {
                min: $min,
                max: $max
            }
        }
    };
}

#[cfg(all(feature = "serde", not(feature = "schemars"), not(feature = "ts-rs")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_range_type_decl {
    ($(#[$meta:meta])* $name:ident : $inner:ty { min: $min:expr, max: $max:expr }) => {
        $crate::__macro_support::__grafton_visca_range_type_decl! {
            crate = $crate;
            [serde]
            $(#[$meta])*
            $name : $inner {
                min: $min,
                max: $max
            }
        }
    };
}

#[cfg(all(feature = "schemars", not(feature = "ts-rs")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_range_type_decl {
    ($(#[$meta:meta])* $name:ident : $inner:ty { min: $min:expr, max: $max:expr }) => {
        $crate::__macro_support::__grafton_visca_range_type_decl! {
            crate = $crate;
            [serde, schemars]
            $(#[$meta])*
            $name : $inner {
                min: $min,
                max: $max
            }
        }
    };
}

#[cfg(all(feature = "ts-rs", not(feature = "schemars")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_range_type_decl {
    ($(#[$meta:meta])* $name:ident : $inner:ty { min: $min:expr, max: $max:expr }) => {
        $crate::__macro_support::__grafton_visca_range_type_decl! {
            crate = $crate;
            [serde, ts_rs]
            $(#[$meta])*
            $name : $inner {
                min: $min,
                max: $max
            }
        }
    };
}

#[cfg(all(feature = "schemars", feature = "ts-rs"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __grafton_visca_range_type_decl {
    ($(#[$meta:meta])* $name:ident : $inner:ty { min: $min:expr, max: $max:expr }) => {
        $crate::__macro_support::__grafton_visca_range_type_decl! {
            crate = $crate;
            [serde, schemars, ts_rs]
            $(#[$meta])*
            $name : $inner {
                min: $min,
                max: $max
            }
        }
    };
}
