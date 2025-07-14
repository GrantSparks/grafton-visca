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
            const MAX_SIZE: usize = 32; // Conservative default

            fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                // Create a helper function for each variant
                $(
                    #[allow(non_snake_case)]
                    fn $variant($($($param: &$ptype),*)?) -> Result<Vec<u8>, $crate::Error> {
                        $body
                    }
                )+

                // Match on self and call the appropriate helper
                let bytes = match self {
                    $(
                        Self::$variant$( ($($param),*) )? => $variant($($($param),*)?)?,
                    )+
                };

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
                use $crate::timeout::CommandCategory;
                match $category {
                    "Quick" => CommandCategory::Quick,
                    "Movement" => CommandCategory::Movement,
                    "Preset" => CommandCategory::Preset,
                    "Custom" => CommandCategory::Custom,
                    _ => CommandCategory::Custom,
                }
            }
        }
    };
}

/// Create a bounded parameter type with validation.
///
/// This macro generates a newtype wrapper that enforces value constraints
/// at the type level.
///
/// # Example
/// ```
/// use grafton_visca::visca_bounded_param;
///
/// visca_bounded_param! {
///     /// Zoom speed level from 0 (slow) to 7 (fast)
///     struct ZoomSpeed: u8 {
///         min: 0,
///         max: 7,
///     }
/// }
/// ```
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
                        value: format!("{}", value),
                        reason: format!("must be between {} and {}", Self::MIN, Self::MAX),
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

/// Validate multiple parameters in a single expression.
///
/// Returns early with an error if any validation fails.
///
/// # Example
/// ```ignore
/// validate_all! {
///     "pan_speed" => pan_speed <= 0x18,
///     "tilt_speed" => tilt_speed <= 0x18,
/// }
/// ```
#[macro_export]
macro_rules! validate_all {
    ($($name:literal => $condition:expr),+ $(,)?) => {
        $(
            if !($condition) {
                return Err($crate::Error::InvalidParameter {
                    parameter: $name,
                    value: format!("{:?}", $name),
                    reason: concat!("failed condition: ", stringify!($condition)).to_string(),
                });
            }
        )+
    };
}

/// Create a boolean command with on/off states.
///
/// This macro simplifies creating commands that toggle features.
///
/// # Example
/// ```
/// use grafton_visca::visca_bool_command;
///
/// visca_bool_command! {
///     /// Control camera backlight compensation
///     struct BacklightCommand {
///         prefix: [0x81, 0x01, 0x04, 0x33],
///         on: 0x02,
///         off: 0x03,
///     }
/// }
/// ```
#[macro_export]
macro_rules! visca_bool_command {
    (
        $(#[$meta:meta])*
        struct $name:ident {
            prefix: [$($prefix:expr),+],
            on: $on:expr,
            off: $off:expr,
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            enabled: bool,
        }

        impl $name {
            /// Creates a new instance with the specified enabled state.
            pub fn new(enabled: bool) -> Self {
                Self { enabled }
            }

            /// Creates a new instance with enabled state set to true.
            pub fn on() -> Self {
                Self { enabled: true }
            }

            /// Creates a new instance with enabled state set to false.
            pub fn off() -> Self {
                Self { enabled: false }
            }
        }

        impl $crate::command::encode_visca::EncodeVisca for $name {
            type Response = ();
            const MAX_SIZE: usize = [$($prefix),+].len() + 2; // prefix + state + 0xFF

            fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let prefix = [$($prefix),+];
                buffer[..prefix.len()].copy_from_slice(&prefix);
                buffer[prefix.len()] = if self.enabled { $on } else { $off };
                buffer[prefix.len() + 1] = 0xFF;

                Ok(Self::MAX_SIZE)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn timeout_kind(&self) -> $crate::timeout::CommandCategory {
                $crate::timeout::CommandCategory::Quick
            }
        }
    };
}

// Internal macro for creating codec implementations
#[doc(hidden)]
#[macro_export]
macro_rules! visca_encode_impl {
    // Standard encode implementation
    (standard $name:ty, $size:literal, $encode_body:expr) => {
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

                let bytes: Vec<u8> = $encode_body(self)?;

                if bytes.len() > Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: bytes.len(),
                        actual: Self::MAX_SIZE,
                    });
                }

                buffer[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn timeout_kind(&self) -> $crate::timeout::CommandCategory {
                $crate::timeout::CommandCategory::Quick
            }
        }
    };
}

/// Internal macro for test generation
#[doc(hidden)]
#[macro_export]
macro_rules! visca_test {
    ($name:ident, $test_name:ident, $cmd:expr, $expected:expr) => {
        #[test]
        fn $test_name() {
            let cmd = $cmd;
            let mut buffer = vec![0u8; 32];
            let len = cmd.encode_into(&mut buffer).expect("encode failed");
            assert_eq!(&buffer[..len], $expected);
        }
    };
}

/// Create a builder-style command with dynamic byte sequences.
///
/// This macro is for commands that need runtime construction of byte sequences
/// based on their parameters.
///
/// # Example
/// ```ignore
/// visca_builder! {
///     /// Set absolute zoom position
///     struct ZoomPosition {
///         position: u16,
///     }
///     builder<9> => |builder, position| {
///         builder.append(&[0x81, 0x01, 0x04, 0x47]);
///         builder.push((position.value() >> 8) as u8);
///         builder.push((position.value() & 0xFF) as u8);
///     }
///     timeout = Movement;
/// }
/// ```
#[macro_export]
macro_rules! visca_builder {
    // Version with fields and visibility
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field:ident: $ftype:ty
            ),+ $(,)?
        }
        builder<$size:literal> => |$builder:ident, $($param:ident),+| {
            $($stmt:stmt);+ $(;)?
        }
        timeout = $category:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name {
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

                let mut $builder = $crate::command::const_encoding::CommandBuilder::<$size>::new();

                // Extract fields and call the builder closure
                {
                    $(let $param = &self.$field;)+
                    $($stmt)*
                }

                let bytes = $builder.build();
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

/// Create a single-byte parameter command.
///
/// This macro generates commands that take a single parameter (typically an enum)
/// and encode it as a single byte in the command sequence.
///
/// # Example
/// ```ignore
/// visca_param_command! {
///     /// Set camera exposure mode
///     pub(crate) struct ExposureCommand {
///         mode: ExposureMode,
///     }
///     prefix = [0x81, 0x01, 0x04, 0x39];
///     param_byte = mode as u8;
///     timeout = Quick;
/// }
/// ```
#[macro_export]
macro_rules! visca_param_command {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $field:ident: $ftype:ty,
        }
        prefix = [$($prefix:expr),+ $(,)?];
        param_byte = $param_expr:expr;
        timeout = $category:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name {
            /// The parameter value.
            pub $field: $ftype,
        }

        impl $crate::command::encode_visca::EncodeVisca for $name {
            type Response = ();
            const MAX_SIZE: usize = [$($prefix),+].len() + 2; // prefix + param + 0xFF

            fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let prefix = [$($prefix),+];
                buffer[..prefix.len()].copy_from_slice(&prefix);

                let $field = &self.$field;
                buffer[prefix.len()] = $param_expr;
                buffer[prefix.len() + 1] = 0xFF;

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

/// Macro to forward method calls from wrapper types to the inner camera instance.
///
/// This macro reduces boilerplate when implementing trait forwarding for the
/// `blocking::Camera` and `async::Camera` wrapper types.
///
/// # Example
/// ```ignore
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
/// ```
#[macro_export]
macro_rules! forward_facade {
    // Blocking variant
    ($wrapper:ident, blocking, $($trait_name:ident : $($method:ident $(($($param:ident : $ptype:ty),* $(,)?))? -> $ret:ty),+ ;)+) => {
        $(
            impl $trait_name for $wrapper {
                $(
                    fn $method(&self $(, $($param: $ptype),*)?) -> $ret {
                        self.0.$method($($($param),*)?)
                    }
                )+
            }
        )+
    };

    // Async variant
    ($wrapper:ident, async, $($trait_name:ident : $($method:ident $(($($param:ident : $ptype:ty),* $(,)?))? -> $ret:ty),+ ;)+) => {
        $(
            impl $trait_name for $wrapper {
                $(
                    async fn $method(&self $(, $($param: $ptype),*)?) -> $ret {
                        self.0.$method($($($param),*)?).await
                    }
                )+
            }
        )+
    };
}
