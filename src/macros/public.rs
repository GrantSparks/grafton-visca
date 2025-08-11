//! Public API macros for extending the grafton-visca library.
//!
//! This module contains macros that are part of the stable public API and are intended
//! for use by library consumers to extend functionality.

/// Create a bounded parameter type with validation.
///
/// This macro generates a newtype wrapper that enforces value constraints
/// at the type level. It's useful for creating custom parameter types that
/// work seamlessly with the VISCA command system.
///
/// # Example
/// ```
/// use grafton_visca::visca_bounded_param;
///
/// visca_bounded_param! {
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
#[macro_export]
macro_rules! visca_bounded_param {
    (
        $(#[$meta:meta])*
        $name:ident : $inner:ty {
            min: $min:expr,
            max: $max:expr
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name($inner);

        impl $name {
            /// Minimum allowed value
            pub const MIN: $inner = $min;
            /// Maximum allowed value
            pub const MAX: $inner = $max;

            /// Create a new instance with validation
            pub fn new(value: $inner) -> Result<Self, $crate::Error> {
                if !(Self::MIN..=Self::MAX).contains(&value) {
                    return Err($crate::Error::InvalidParameter {
                        parameter: stringify!($name),
                        value: ::std::borrow::Cow::Owned(format!("{value}")),
                        reason: {
                            let min = Self::MIN;
                            let max = Self::MAX;
                            ::std::borrow::Cow::Owned(format!("must be between {min} and {max}"))
                        },
                    });
                }
                Ok(Self(value))
            }

            /// Get the inner value
            #[must_use]
            pub fn value(&self) -> $inner {
                self.0
            }
        }

        impl TryFrom<$inner> for $name {
            type Error = $crate::Error;

            fn try_from(value: $inner) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for $inner {
            fn from(val: $name) -> Self {
                val.0
            }
        }
    };
}

/// Macro to forward method calls from wrapper types to the inner camera instance.
///
/// This macro reduces boilerplate when implementing trait forwarding for the
/// `blocking::Camera` and `async::Camera` wrapper types. It's particularly useful
/// when creating custom camera wrappers that need to expose the same API.
///
/// # Example
///
/// ```ignore
/// use grafton_visca::forward_facade;
///
/// // For blocking APIs
/// forward_facade!(Camera, blocking,
///     ZoomOps:
///         zoom_stop() -> crate::Result<()>,
///         zoom_in() -> crate::Result<()>,
///         zoom_out() -> crate::Result<()>,
///         zoom_absolute(position: crate::units::Normalized) -> crate::Result<()>;
///     InquiryOps:
///         get_power_state() -> crate::Result<bool>,
///         get_zoom_position() -> crate::Result<u16>;
/// );
///
/// // For async APIs
/// forward_facade!(AsyncCamera, async,
///     ZoomOps:
///         zoom_stop() -> crate::Result<()>,
///         zoom_in() -> crate::Result<()>;
/// );
/// ```
///
/// # Syntax
///
/// - First parameter: The wrapper type name
/// - Second parameter: Either `blocking` or `async`
/// - Following parameters: Trait implementations with method signatures
///
/// ## Method Disambiguation
///
/// When forwarding methods that might have naming conflicts, you can use the `@` syntax:
///
/// ```ignore
/// forward_facade!(Camera, blocking,
///     PowerOps:
///         power_on() -> crate::Result<()>,
///         get_state @ PowerOps() -> crate::Result<PowerState>;
/// );
/// ```
#[macro_export]
macro_rules! forward_facade {
    // Blocking variant with optional trait disambiguation
    ($wrapper:ident, blocking, $($trait_name:ident : $($method:ident $(@ $disambiguate_trait:ident)? $(($($param:ident : $ptype:ty),* $(,)?))? -> $ret:ty),+ ;)+) => {
        $(
            impl<P: $crate::capabilities::Profile, T: $crate::transport::Transport + Send + Sync + 'static + $crate::transport::core::BlockingTransport> $trait_name for $wrapper<P, T>
            where
                T::Error: Into<$crate::Error> + Send,
                for<'a> T::SendFut<'a>: Send,
                for<'a> T::RecvFut<'a>: Send,
            {
                $(
                    fn $method(&self $(, $($param: $ptype),*)?) -> $ret {
                        forward_facade!(@call $($disambiguate_trait)?, $method, self.0, $($($param),*)?)
                    }
                )+
            }
        )+
    };

    // Async variant with optional trait disambiguation
    ($wrapper:ident, async, $($trait_name:ident : $($method:ident $(@ $disambiguate_trait:ident)? $(($($param:ident : $ptype:ty),* $(,)?))? -> $ret:ty),+ ;)+) => {
        $(
            impl<P: $crate::capabilities::Profile, T: $crate::transport::Transport + Send + Sync + 'static> $trait_name for $wrapper<P, T>
            where
                T::Error: Into<$crate::Error> + Send,
                for<'a> T::SendFut<'a>: Send,
                for<'a> T::RecvFut<'a>: Send,
            {
                $(
                    async fn $method(&self $(, $($param: $ptype),*)?) -> $ret {
                        forward_facade!(@call_async $($disambiguate_trait)?, $method, self.0, $($($param),*)?)
                    }
                )+
            }
        )+
    };

    // Helper for blocking calls with trait disambiguation
    (@call $trait:ident, $method:ident, $receiver:expr, $($param:expr),*) => {
        $trait::$method(&$receiver, $($param),*)
    };

    // Helper for blocking calls without trait disambiguation
    (@call , $method:ident, $receiver:expr, $($param:expr),*) => {
        $receiver.$method($($param),*)
    };

    // Helper for async calls with trait disambiguation
    (@call_async $trait:ident, $method:ident, $receiver:expr, $($param:expr),*) => {
        $trait::$method(&$receiver, $($param),*).await
    };

    // Helper for async calls without trait disambiguation
    (@call_async , $method:ident, $receiver:expr, $($param:expr),*) => {
        $receiver.$method($($param),*).await
    };
}
