//! Declarative macros for reducing VISCA command boilerplate.
//!
//! This module provides macros to simplify the creation of VISCA commands,
//! reducing repetitive code while maintaining type safety and clarity.
//!
//! ## Available Macros
//!
//! - `visca_command!` - Create simple command enums
//! - `visca_bounded_param!` - Create validated newtype wrappers for numeric parameters
//! - `visca_bool_command!` - Create commands with boolean on/off parameters

/// Create a simple VISCA command enum with byte sequences.
///
/// This macro generates a complete implementation of the `Command` trait
/// for simple commands that don't require parameters.
///
/// # Example
/// ```ignore
/// // Internal macro - not part of public API
///
/// visca_command! {
///     category = "Movement",
///     enum PanTilt {
///         Home => {
///             Ok(vec![0x81, 0x01, 0x06, 0x04, 0xFF])
///         },
///         Reset => {
///             Ok(vec![0x81, 0x01, 0x06, 0x05, 0xFF])
///         },
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

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                // Create a helper function for each variant
                $(
                    #[allow(non_snake_case)]
                    fn $variant($($($param: &$ptype),*)?) -> Result<Vec<u8>, $crate::Error> {
                        $body
                    }
                )+

                // Match on self and call the appropriate helper
                let mut bytes = match self {
                    $(
                        Self::$variant$( ($($param),*) )? => $variant($($($param),*)?)?,
                    )+
                };

                // Replace hardcoded camera ID with dynamic one
                if !bytes.is_empty() && bytes[0] == 0x81 {
                    bytes[0] = camera_id.to_address_byte();
                }

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
///     ZoomSpeed: u8 {
///         min: 0,
///         max: 7
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

/// Create a boolean command with on/off states.
///
/// This macro simplifies creating commands that toggle features.
///
/// # Example
/// ```ignore
/// // Internal macro - not part of public API
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
            pub(crate) fn new(enabled: bool) -> Self {
                Self { enabled }
            }
        }

        impl $crate::command::encode_visca::EncodeVisca for $name {
            type Response = ();
            const MAX_SIZE: usize = [$($prefix),+].len() + 2; // prefix + state + 0xFF

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let mut prefix = [$($prefix),+];
                // Replace hardcoded camera ID with dynamic one
                if !prefix.is_empty() && prefix[0] == 0x81 {
                    prefix[0] = camera_id.to_address_byte();
                }

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

/// Internal macro for test generation
#[doc(hidden)]
#[macro_export]
macro_rules! visca_test {
    ($name:ident, $test_name:ident, $cmd:expr, $expected:expr) => {
        #[test]
        fn $test_name() {
            use $crate::camera_id::CameraId;
            use $crate::command::encode_visca::EncodeVisca;
            let cmd = $cmd;
            let mut buffer = vec![0u8; 32];
            let len = cmd
                .encode_into(CameraId::CAMERA_1, &mut buffer)
                .expect("encode failed");
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

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
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
                // Replace hardcoded camera ID with dynamic one
                if buffer[0] == 0x81 {
                    buffer[0] = camera_id.to_address_byte();
                }
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

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let mut prefix = [$($prefix),+];
                // Replace hardcoded camera ID with dynamic one
                if !prefix.is_empty() && prefix[0] == 0x81 {
                    prefix[0] = camera_id.to_address_byte();
                }
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
    // Blocking variant with optional trait disambiguation
    ($wrapper:ident, blocking, $($trait_name:ident : $($method:ident $(@ $disambiguate_trait:ident)? $(($($param:ident : $ptype:ty),* $(,)?))? -> $ret:ty),+ ;)+) => {
        $(
            impl<P: $crate::capabilities::Profile, T: $crate::transport::UnifiedTransport> $trait_name for $wrapper<P, T> {
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
            impl<P: $crate::capabilities::Profile, T: $crate::transport::UnifiedTransport> $trait_name for $wrapper<P, T> {
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
