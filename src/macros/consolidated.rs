//! Consolidated macros for VISCA command implementation.
//!
//! This module provides the unified macro interface as specified in issue #397.
//! Primary macro: `visca_command!` for commands.

/// Create a VISCA command that expects ACK/Completion responses.
///
/// # Examples
///
/// Simple command without parameters:
/// ```ignore
/// visca_command! {
///     /// Power on the camera
///     pub struct PowerOn;
///     bytes = [0x01, 0x04, 0x00, 0x02];
///     category = CommandCategory::Quick;
/// }
/// ```
///
/// Command with parameters:
/// ```ignore
/// visca_command! {
///     pub struct ImageFlip { mode: Flip };
///     prefix = [0x01, 0x06, 0x61];
///     param = match mode { Flip::On => 0x02, Flip::Off => 0x03 };
///     category = CommandCategory::Quick;
/// }
/// ```
#[macro_export]
macro_rules! visca_command {
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

            fn response_kind(&self) -> Option<$crate::command::InquiryKind> {
                None
            }
        }
    };

    // Command with parameters and explicit max_param_size
    (
        $(#[$meta:meta])*
        pub struct $name:ident { $($field:ident : $ftype:ty),* $(,)? };
        prefix = [$($byte:expr),* $(,)?];
        param = $param_expr:expr;
        max_param_size = $max_param_size:expr;
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(/// The parameter value.
            pub $field: $ftype,)*
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = 1 + [$($byte),*].len() + $max_param_size + 1;
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {


                const PREFIX: &[u8] = &[$($byte),*];

                // Destructure self for use in param expression
                let Self { $($field),* } = self;

                // Evaluate the param expression and convert to Vec<u8>
                let param_bytes: Vec<u8> = $crate::macros::consolidated::to_param_bytes($param_expr);

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

            fn response_kind(&self) -> Option<$crate::command::InquiryKind> {
                None
            }
        }
    };

    // Command with parameters (without explicit max_param_size, default to 1)
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
            $(/// The parameter value.
            pub $field: $ftype,)*
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            // Default to 1 byte for simple parameters
            const MAX_SIZE: usize = 1 + [$($byte),*].len() + 1 + 1;
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {


                const PREFIX: &[u8] = &[$($byte),*];

                // Destructure self for use in param expression
                let Self { $($field),* } = self;

                // Evaluate the param expression and convert to Vec<u8>
                let param_bytes: Vec<u8> = $crate::macros::consolidated::to_param_bytes($param_expr);

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

            fn response_kind(&self) -> Option<$crate::command::InquiryKind> {
                None
            }
        }
    };
}

/// Helper trait to convert various types to `Vec<u8>` for the visca_command! macro.
pub trait ToParamBytes {
    fn to_param_bytes(self) -> Vec<u8>;
}

impl ToParamBytes for u8 {
    fn to_param_bytes(self) -> Vec<u8> {
        vec![self]
    }
}

impl ToParamBytes for Vec<u8> {
    fn to_param_bytes(self) -> Vec<u8> {
        self
    }
}

impl<const N: usize> ToParamBytes for [u8; N] {
    fn to_param_bytes(self) -> Vec<u8> {
        self.to_vec()
    }
}

/// Convert various types to `Vec<u8>` for use in visca_command! macro parameters.
pub fn to_param_bytes<T: ToParamBytes>(value: T) -> Vec<u8> {
    value.to_param_bytes()
}
