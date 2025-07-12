//! Declarative macros for reducing VISCA command boilerplate.
//!
//! This module provides macros to simplify the creation of VISCA commands,
//! reducing repetitive code while maintaining type safety and clarity.
//!
//! ## Available Macros
//!
//! - `visca_command!` - Create simple command enums
//! - `visca_bounded_param!` - Create validated newtype wrappers for numeric parameters
//! - `validate_all!` - Validate multiple parameters in a single expression
//! - `visca_bool_command!` - Create commands with boolean on/off parameters

/// Create a simple VISCA command enum with byte sequences.
///
/// This macro generates a complete implementation of the `Command` trait
/// for simple commands that don't require parameters.
///
/// # Example
/// ```
/// use grafton_visca::visca_command;
///
/// visca_command! {
///     category = "Movement",
///     enum PanTilt {
///         Home => [0x81, 0x01, 0x06, 0x04, 0xFF],
///         Reset => [0x81, 0x01, 0x06, 0x05, 0xFF],
///     }
/// }
/// ```
#[macro_export]
macro_rules! visca_command {
    (
        $(#[$meta:meta])*
        category = $category:literal,
        enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident $( ($($param:ident : $ptype:ty),*) )? => $body:tt
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant $( ($($ptype),*) )?,
            )+
        }

        impl $crate::command::Command for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::Error> {
                match self {
                    $(
                        Self::$variant $( ($($param),*) )? => $crate::visca_command!(@expand_body $body, $($($param),*)?),
                    )+
                }
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                match $category {
                    "Quick" => $crate::timeout::CommandCategory::Quick,
                    "Movement" => $crate::timeout::CommandCategory::Movement,
                    "Preset" => $crate::timeout::CommandCategory::Preset,
                    "LongRunning" => $crate::timeout::CommandCategory::LongRunning,
                    "Custom" => $crate::timeout::CommandCategory::Custom,
                    _ => $crate::timeout::CommandCategory::Custom,
                }
            }
        }
    };

    // Expand simple byte arrays
    (@expand_body [$($byte:expr),+ $(,)?], $($params:ident)*) => {
        Ok(vec![$($byte),+])
    };

    // Expand code blocks
    (@expand_body $block:block, $($params:ident)*) => {
        $block
    };
}

/// Create a validated newtype wrapper for numeric parameters.
///
/// This macro generates a newtype struct with validation, conversion traits,
/// and common methods for VISCA parameter types that have min/max bounds.
///
/// # Example
/// ```ignore
/// use grafton_visca::visca_bounded_param;
///
/// visca_bounded_param! {
///     /// Variable zoom speed (0-7).
///     ZoomSpeed: u8 {
///         min: 0,
///         max: 7,
///         error_msg: "Zoom speed must be in the range 0..=7"
///     }
/// }
/// ```
#[macro_export]
macro_rules! visca_bounded_param {
    (
        $(#[$meta:meta])*
        $name:ident: $type:ty {
            min: $min:expr,
            max: $max:expr,
            error_msg: $error_msg:expr
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name($type);

        impl $name {
            /// Minimum allowed value.
            pub const MIN: $type = $min;
            /// Maximum allowed value.
            pub const MAX: $type = $max;

            /// Creates a new instance with validation.
            ///
            /// # Errors
            /// Returns `Error::InvalidParameter` if value is out of range.
            pub fn new(value: $type) -> Result<Self, $crate::Error> {
                if (Self::MIN..=Self::MAX).contains(&value) {
                    Ok(Self(value))
                } else {
                    Err($crate::Error::InvalidParameter($error_msg.into()))
                }
            }

            /// Get the raw value.
            #[must_use]
            pub const fn value(self) -> $type {
                self.0
            }
        }

        impl std::convert::TryFrom<$type> for $name {
            type Error = $crate::Error;

            fn try_from(value: $type) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for $type {
            fn from(val: $name) -> Self {
                val.0
            }
        }
    };
}

/// Validate multiple parameters in a single expression.
///
/// This macro simplifies the common pattern of validating multiple parameters
/// with consistent error handling. It converts validation errors to InvalidParameter
/// errors with descriptive messages.
///
/// # Example
/// ```ignore
/// use grafton_visca::validate_all;
///
/// fn continuous_move(pan_speed: u8, tilt_speed: u8) -> Result<Self, Error> {
///     let (pan_speed, tilt_speed) = validate_all! {
///         pan_speed: PanSpeed::new(pan_speed),
///         tilt_speed: TiltSpeed::new(tilt_speed),
///     }?;
///     
///     Ok(Self::Move {
///         direction: PanTiltDirection::Up,
///         pan_speed,
///         tilt_speed,
///     })
/// }
/// ```
#[macro_export]
macro_rules! validate_all {
    (
        $($field:ident : $constructor:expr),+ $(,)?
    ) => {{
        {
            $(
                let $field = $constructor.map_err(|_| {
                    $crate::Error::InvalidParameter(
                        concat!("Invalid ", stringify!($field)).to_string()
                    )
                })?;
            )+
            Ok::<_, $crate::Error>(($($field),+))
        }
    }};
}

/// Create VISCA commands with boolean on/off parameters.
///
/// This macro generates commands that have a simple boolean parameter
/// for enabling/disabling features.
///
/// # Example
/// ```
/// use grafton_visca::visca_bool_command;
///
/// visca_bool_command! {
///     /// Enable or disable backlight compensation.
///     struct BacklightCommand {
///         /// Enable (true) or disable (false) backlight compensation.
///         status: bool => |v| if v { 0x02 } else { 0x03 }
///     }
///     bytes = [0x81, 0x01, 0x04, 0x33, {status}, 0xFF]
/// }
/// ```
#[macro_export]
macro_rules! visca_bool_command {
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $(#[$field_meta:meta])*
            $field:ident: bool => |$v:ident| $encode:expr
        }
        bytes = [$($byte:tt)*]
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(#[$field_meta])*
            pub $field: bool,
        }

        impl $crate::command::Command for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::Error> {
                let $v = self.$field;
                let encoded = $encode;
                let mut bytes = Vec::with_capacity(16);
                $crate::visca_bool_command!(@encode bytes, encoded, [$($byte)*]);
                Ok(bytes)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                $crate::timeout::CommandCategory::Quick
            }
        }
    };

    // Internal encoding rules
    (@encode $bytes:ident, $encoded:ident, []) => {};

    (@encode $bytes:ident, $encoded:ident, [$byte:literal, $($rest:tt)*]) => {
        $bytes.push($byte);
        $crate::visca_bool_command!(@encode $bytes, $encoded, [$($rest)*]);
    };

    (@encode $bytes:ident, $encoded:ident, [$byte:literal]) => {
        $bytes.push($byte);
    };

    (@encode $bytes:ident, $encoded:ident, [{$field:ident}, $($rest:tt)*]) => {
        $bytes.push($encoded);
        $crate::visca_bool_command!(@encode $bytes, $encoded, [$($rest)*]);
    };

    (@encode $bytes:ident, $encoded:ident, [{$field:ident}]) => {
        $bytes.push($encoded);
    };
}
