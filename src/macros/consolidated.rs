//! Consolidated macros for VISCA command implementation.
//!
//! This module provides the unified macro interface as specified in issue #397.
//! Two primary macros: `visca_cmd!` for commands and `visca_inquiry!` for inquiries.

/// Create a VISCA command that expects ACK/Completion responses.
///
/// # Examples
///
/// Simple command without parameters:
/// ```ignore
/// visca_cmd! {
///     /// Power on the camera
///     pub struct PowerOn;
///     bytes = [0x01, 0x04, 0x00, 0x02];
///     category = CommandCategory::Quick;
/// }
/// ```
///
/// Command with parameters:
/// ```ignore
/// visca_cmd! {
///     pub struct ImageFlip { mode: Flip };
///     prefix = [0x01, 0x06, 0x61];
///     param = match mode { Flip::On => 0x02, Flip::Off => 0x03 };
///     category = CommandCategory::Quick;
/// }
/// ```
#[macro_export]
macro_rules! visca_cmd {
    // Simple command without parameters
    (
        $(#[$meta:meta])*
        pub struct $name:ident;
        bytes = [$($byte:expr),* $(,)?];
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name;

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = { [$($byte),*].len() + 2 }; // +2 for camera_id and terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                const BYTES: &[u8] = &[$($byte),*];
                let len = BYTES.len() + 2;

                if buffer.len() < len {
                    return Err($crate::Error::BufferTooSmall {
                        required: len,
                        actual: buffer.len()
                    });
                }

                buffer[0] = camera_id.to_address_byte();
                buffer[1..1+BYTES.len()].copy_from_slice(BYTES);
                buffer[len-1] = $crate::command::bytes::VISCA_TERMINATOR;
                Ok(len)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                None
            }
        }
    };

    // Command with parameters
    (
        $(#[$meta:meta])*
        pub struct $name:ident { $($field:ident : $ftype:ty),* $(,)? };
        prefix = [$($byte:expr),* $(,)?];
        param = $param_expr:expr;
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(pub $field: $ftype,)*
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = 16; // Conservative estimate for parameterized commands
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                use $crate::command::bytes::ConstCommandBuilder;

                const PREFIX: &[u8] = &[$($byte),*];

                // Destructure self for use in param expression
                let Self { $($field),* } = self;

                // Evaluate the param expression
                let param_bytes: Vec<u8> = $param_expr;

                let total_len = 1 + PREFIX.len() + param_bytes.len() + 1; // camera_id + prefix + params + terminator

                if buffer.len() < total_len {
                    return Err($crate::Error::BufferTooSmall {
                        required: total_len,
                        actual: buffer.len()
                    });
                }

                let mut pos = 0;
                buffer[pos] = camera_id.to_address_byte();
                pos += 1;

                buffer[pos..pos+PREFIX.len()].copy_from_slice(PREFIX);
                pos += PREFIX.len();

                buffer[pos..pos+param_bytes.len()].copy_from_slice(&param_bytes);
                pos += param_bytes.len();

                buffer[pos] = $crate::command::bytes::VISCA_TERMINATOR;
                Ok(pos + 1)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                None
            }
        }
    };
}

/// Create a VISCA inquiry that expects a data response.
///
/// # Examples
///
/// ```ignore
/// visca_inquiry! {
///     /// Current zoom position
///     pub struct ZoomPosition;
///     prefix = [0x09, 0x04, 0x47];
///     returns = ResponseKind::ZoomPosition;
///     category = CommandCategory::Quick;
/// }
/// ```
#[macro_export]
macro_rules! visca_inquiry {
    (
        $(#[$meta:meta])*
        pub struct $name:ident;
        prefix = [$($byte:expr),* $(,)?];
        returns = $response:expr;
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name;

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = { [$($byte),*].len() + 2 }; // +2 for camera_id and terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                const PREFIX: &[u8] = &[$($byte),*];
                let len = PREFIX.len() + 2;

                if buffer.len() < len {
                    return Err($crate::Error::BufferTooSmall {
                        required: len,
                        actual: buffer.len()
                    });
                }

                buffer[0] = camera_id.to_address_byte();
                buffer[1..1+PREFIX.len()].copy_from_slice(PREFIX);
                buffer[len-1] = $crate::command::bytes::VISCA_TERMINATOR;
                Ok(len)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                Some($response)
            }
        }
    };

    // Inquiry with parameters
    (
        $(#[$meta:meta])*
        pub struct $name:ident { $($field:ident : $ftype:ty),* $(,)? };
        prefix = [$($byte:expr),* $(,)?];
        param = $param_expr:expr;
        returns = $response:expr;
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(pub $field: $ftype,)*
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = 16; // Conservative estimate
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                const PREFIX: &[u8] = &[$($byte),*];

                // Destructure self for use in param expression
                let Self { $($field),* } = self;

                // Evaluate the param expression
                let param_bytes: Vec<u8> = $param_expr;

                let total_len = 1 + PREFIX.len() + param_bytes.len() + 1; // camera_id + prefix + params + terminator

                if buffer.len() < total_len {
                    return Err($crate::Error::BufferTooSmall {
                        required: total_len,
                        actual: buffer.len()
                    });
                }

                let mut pos = 0;
                buffer[pos] = camera_id.to_address_byte();
                pos += 1;

                buffer[pos..pos+PREFIX.len()].copy_from_slice(PREFIX);
                pos += PREFIX.len();

                buffer[pos..pos+param_bytes.len()].copy_from_slice(&param_bytes);
                pos += param_bytes.len();

                buffer[pos] = $crate::command::bytes::VISCA_TERMINATOR;
                Ok(pos + 1)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                Some($response)
            }
        }
    };
}

/// Create a type with range validation.
///
/// This macro was renamed from `visca_bounded_param!` to better reflect
/// that it creates a type, not just a parameter.
///
/// # Examples
///
/// ```ignore
/// visca_range_type! {
///     /// Focus speed from 0-7
///     pub struct FocusSpeed(u8, 0..=7);
/// }
/// ```
#[macro_export]
macro_rules! visca_range_type {
    (
        $(#[$meta:meta])*
        pub struct $name:ident($inner:ty, $min:literal..=$max:literal);
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name($inner);

        impl $name {
            /// The minimum valid value
            pub const MIN: $inner = $min;
            /// The maximum valid value
            pub const MAX: $inner = $max;

            /// Create a new instance with range validation
            pub fn new(value: $inner) -> Result<Self, $crate::Error> {
                if value < Self::MIN || value > Self::MAX {
                    Err($crate::Error::InvalidParameter(
                        format!(
                            concat!(stringify!($name), " value {} out of range [{}, {}]"),
                            value, Self::MIN, Self::MAX
                        ).into()
                    ))
                } else {
                    Ok(Self(value))
                }
            }

            /// Create without validation (unsafe)
            pub const fn new_unchecked(value: $inner) -> Self {
                Self(value)
            }

            /// Get the inner value
            pub const fn value(&self) -> $inner {
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
