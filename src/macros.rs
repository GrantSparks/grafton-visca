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

        impl $crate::command::encode_visca::EncodeVisca for $name {
            type Response = ();
            const MAX_SIZE: usize = 16; // Conservative size for enum variants

            fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                let bytes = match self {
                    $(
                        Self::$variant $( ($($param),*) )? => $crate::visca_command!(@expand_body $body, $($($param),*)?),
                    )+
                }?;

                if buffer.len() < bytes.len() {
                    return Err($crate::Error::BufferTooSmall {
                        required: bytes.len(),
                        actual: buffer.len(),
                    });
                }

                buffer[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn timeout_kind(&self) -> $crate::timeout::CommandCategory {
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

    // Expand builder pattern - creates CommandBuilder and builds with terminator
    (@expand_body builder ( $const_path:path ), $($params:ident)*) => {{
        let cmd = $crate::command::const_encoding::CommandBuilder::<6>::new()
            .append($const_path)
            .build();
        Ok(cmd.to_vec())
    }};
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

        impl $crate::command::encode_visca::EncodeVisca for $name {
            type Response = ();
            const MAX_SIZE: usize = 6; // Most VISCA commands are 6 bytes

            fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let $v = self.$field;
                let encoded = $encode;
                let mut bytes = Vec::with_capacity(Self::MAX_SIZE);
                $crate::visca_bool_command!(@encode bytes, encoded, [$($byte)*]);

                let len = bytes.len();
                buffer[..len].copy_from_slice(&bytes);
                Ok(len)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn timeout_kind(&self) -> $crate::timeout::CommandCategory {
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

/// Create a VISCA command from a static byte array.
///
/// This macro generates a complete implementation of the `EncodeVisca` trait
/// for commands that always encode to the same byte sequence.
///
/// # Example
/// ```ignore
/// use grafton_visca::static_visca_cmd;
///
/// static_visca_cmd! {
///     /// Power on command.
///     struct PowerOn = [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];
///     timeout = Quick;
/// }
/// ```
#[macro_export]
macro_rules! static_visca_cmd {
    (
        $(#[$meta:meta])*
        struct $name:ident = [$($byte:expr),+ $(,)?];
        timeout = $category:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name;

        impl $crate::command::encode_visca::EncodeVisca for $name {
            type Response = ();
            const MAX_SIZE: usize = { 0 $(+ { let _ = $byte; 1 })+ };

            fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                const BYTES: &[u8] = &[$($byte),+];

                if buffer.len() < BYTES.len() {
                    return Err($crate::Error::BufferTooSmall {
                        required: BYTES.len(),
                        actual: buffer.len(),
                    });
                }

                buffer[..BYTES.len()].copy_from_slice(BYTES);
                Ok(BYTES.len())
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn timeout_kind(&self) -> $crate::timeout::CommandCategory {
                $crate::timeout::CommandCategory::$category
            }
        }
    };
}

/// Create a VISCA command using a builder pattern.
///
/// This macro generates commands that have a common prefix followed by
/// variable data, using the CommandBuilder pattern internally.
///
/// # Example
/// ```ignore
/// use grafton_visca::visca_builder;
///
/// visca_builder! {
///     /// Zoom to a specific position.
///     struct ZoomDirect {
///         position: ZoomPosition,
///     }
///     builder<6> {
///         append([0x81, 0x01, 0x04, 0x47]);
///         encode_u16(position.value());
///     }
///     timeout = Movement;
/// }
/// ```
#[macro_export]
macro_rules! visca_builder {
    // Version with fields
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field:ident: $ftype:ty
            ),+ $(,)?
        }
        builder<$size:literal> {
            $($stmt:stmt);+ $(;)?
        }
        timeout = $category:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(
                $(#[$field_meta])*
                pub $field: $ftype,
            )+
        }

        impl $crate::command::encode_visca::EncodeVisca for $name {
            type Response = ();
            const MAX_SIZE: usize = $size;

            fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let mut builder = $crate::command::const_encoding::CommandBuilder::<$size>::new();

                // Extract fields for use in the builder block
                $(
                    let $field = &self.$field;
                )+

                // Execute the builder statements
                $($stmt);+

                let bytes = builder.build();
                buffer[..Self::MAX_SIZE].copy_from_slice(&bytes);
                Ok(Self::MAX_SIZE)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn timeout_kind(&self) -> $crate::timeout::CommandCategory {
                $crate::timeout::CommandCategory::$category
            }
        }
    };
}
