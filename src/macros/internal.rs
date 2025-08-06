//! Internal macros for VISCA command implementation.
//!
//! This module contains macros used internally by the library for implementing
//! VISCA commands. These macros are not part of the public API and may change
//! without notice.
//!
//! ## Available Macros
//!
//! - Command generators: `visca_command!`, `visca_bool_command!`, `visca_builder!`,
//!   `visca_param_command!`, `visca_const_command!`
//! - Const utilities: `visca_bytes!`, `visca_prefix!`

/// Convert a string literal to a CommandCategory at compile time
pub const fn str_to_command_category(s: &str) -> crate::timeout::CommandCategory {
    use crate::timeout::CommandCategory;
    match str_bytes(s) {
        b"Quick" => CommandCategory::Quick,
        b"Movement" => CommandCategory::Movement,
        b"Preset" => CommandCategory::Preset,
        b"Custom" => CommandCategory::Custom,
        b"Network" => CommandCategory::Network,
        _ => CommandCategory::Custom,
    }
}

/// Convert &str to &[u8] at compile time
const fn str_bytes(s: &str) -> &[u8] {
    s.as_bytes()
}

/// Create a simple VISCA command enum with byte sequences.
///
/// This macro generates a complete implementation of the `Command` trait
/// for simple commands that don't require parameters.
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
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory =
                $crate::macros::internal::str_to_command_category($category);

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                $(
                    #[allow(non_snake_case)]
                    fn $variant($($($param: &$ptype),*)?) -> Result<Vec<u8>, $crate::Error> {
                        $body
                    }
                )+

                let mut bytes = match self {
                    $(
                        Self::$variant$( ($($param),*) )? => $variant($($($param),*)?)?,
                    )+
                };

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
        }
    };
}

/// Create a boolean command with on/off states.
///
/// This macro simplifies creating commands that toggle features.
macro_rules! visca_bool_command {
    // Original form without optional parameters
    (
        $(#[$meta:meta])*
        struct $name:ident {
            prefix: [$($prefix:expr),+],
            on: $on:expr,
            off: $off:expr,
        }
    ) => {
        visca_bool_command! {
            $(#[$meta])*
            struct $name {
                prefix: [$($prefix),+],
                on: $on,
                off: $off,
                address: 0x81,
                response: None,
            }
        }
    };

    // Extended form with optional parameters
    (
        $(#[$meta:meta])*
        struct $name:ident {
            prefix: [$($prefix:expr),+],
            on: $on:expr,
            off: $off:expr,
            address: $address:expr,
            response: $response:expr,
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
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::Quick;

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let mut prefix = [$($prefix),+];
                if !prefix.is_empty() && prefix[0] == $address {
                    prefix[0] = ($address & 0xF0) | camera_id.id();
                }

                buffer[..prefix.len()].copy_from_slice(&prefix);
                buffer[prefix.len()] = if self.enabled { $on } else { $off };
                buffer[prefix.len() + 1] = 0xFF;

                Ok(Self::MAX_SIZE)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                $response
            }
        }
    };
}

/// Create a builder-style command with dynamic byte sequences.
///
/// This macro is for commands that need runtime construction of byte sequences
/// based on their parameters.
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
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let mut $builder = $crate::command::const_encoding::CommandBuilder::<$size>::new();

                {
                    $(let $param = &self.$field;)+
                    $($stmt)*
                }

                let bytes = $builder.build();
                buffer[..Self::MAX_SIZE].copy_from_slice(&bytes);
                if buffer[0] == 0x81 {
                    buffer[0] = camera_id.to_address_byte();
                }
                Ok(Self::MAX_SIZE)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }
        }
    };
}

/// Create a single-byte parameter command.
///
/// This macro generates commands that take a single parameter (typically an enum)
/// and encode it as a single byte in the command sequence.
macro_rules! visca_param_command {
    // Original form without optional parameters - array literal
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $field:ident: $ftype:ty,
        }
        prefix = [$($prefix:expr),+ $(,)?];
        param_byte = $param_expr:expr;
        timeout = $category:ident;
    ) => {
        visca_param_command! {
            $(#[$meta])*
            $vis struct $name {
                $field: $ftype,
            }
            prefix = [$($prefix),+];
            param_byte = $param_expr;
            timeout = $category;
            address = 0x81;
            response = None;
        }
    };

    // New form without optional parameters - constant reference
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $field:ident: $ftype:ty,
        }
        prefix = $prefix_const:expr;
        param_byte = $param_expr:expr;
        timeout = $category:ident;
    ) => {
        visca_param_command! {
            $(#[$meta])*
            $vis struct $name {
                $field: $ftype,
            }
            prefix_const = $prefix_const;
            param_byte = $param_expr;
            timeout = $category;
            address = 0x81;
            response = None;
        }
    };

    // Extended form with optional parameters - array literal
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $field:ident: $ftype:ty,
        }
        prefix = [$($prefix:expr),+ $(,)?];
        param_byte = $param_expr:expr;
        timeout = $category:ident;
        address = $address:expr;
        response = $response:expr;
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
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let mut prefix = [$($prefix),+];
                if !prefix.is_empty() && prefix[0] == $address {
                    prefix[0] = ($address & 0xF0) | camera_id.id();
                }
                buffer[..prefix.len()].copy_from_slice(&prefix);

                let $field = &self.$field;
                buffer[prefix.len()] = $param_expr;
                buffer[prefix.len() + 1] = 0xFF;

                Ok(Self::MAX_SIZE)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                $response
            }

        }
    };

    // Extended form with optional parameters - constant reference
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $field:ident: $ftype:ty,
        }
        prefix_const = $prefix_const:expr;
        param_byte = $param_expr:expr;
        timeout = $category:ident;
        address = $address:expr;
        response = $response:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name {
            /// The parameter value.
            pub $field: $ftype,
        }

        impl $crate::command::encode_visca::EncodeVisca for $name {
            type Response = ();
            const MAX_SIZE: usize = $prefix_const.len() + 2; // prefix + param + 0xFF
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                if buffer.len() < Self::MAX_SIZE {
                    return Err($crate::Error::BufferTooSmall {
                        required: Self::MAX_SIZE,
                        actual: buffer.len(),
                    });
                }

                let prefix_bytes = $prefix_const;
                let mut prefix = [0u8; 16]; // Max reasonable prefix size
                prefix[..prefix_bytes.len()].copy_from_slice(prefix_bytes);

                if prefix_bytes.len() > 0 && prefix_bytes[0] == $address {
                    prefix[0] = ($address & 0xF0) | camera_id.id();
                }
                buffer[..prefix_bytes.len()].copy_from_slice(&prefix[..prefix_bytes.len()]);

                let $field = &self.$field;
                buffer[prefix_bytes.len()] = $param_expr;
                buffer[prefix_bytes.len() + 1] = 0xFF;

                Ok(prefix_bytes.len() + 2)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                $response
            }

        }
    };
}

/// Create a simple constant byte command.
///
/// This macro generates commands that always send the same fixed byte sequence,
/// typically used for commands with no parameters like "Cancel", "Menu Open", etc.
macro_rules! visca_const_command {
    // Original form without optional parameters
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        bytes = [$($byte:expr),+ $(,)?];
        timeout = $category:ident;
    ) => {
        visca_const_command! {
            $(#[$meta])*
            $vis struct $name;
            bytes = [$($byte),+];
            timeout = $category;
            address = 0x81;
            response = None;
        }
    };

    // Extended form with optional parameters
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        bytes = [$($byte:expr),+ $(,)?];
        timeout = $category:ident;
        address = $address:expr;
        response = $response:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name;

        impl $name {
            /// Create a new instance of this command.
            pub const fn new() -> Self {
                Self
            }

            /// Command bytes as a const array (without terminator).
            const BYTES: &'static [u8] = &[$($byte),+];
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::command::encode_visca::EncodeVisca for $name {
            type Response = ();
            const MAX_SIZE: usize = { [$($byte),+].len() };
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn encode_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::error::Error> {
                const BYTES: &[u8] = $name::BYTES;
                const LEN: usize = BYTES.len();

                if buffer.len() < LEN {
                    return Err($crate::error::Error::BufferTooSmall {
                        required: LEN,
                        actual: buffer.len(),
                    });
                }

                buffer[..LEN].copy_from_slice(BYTES);

                if $address == 0x81 && BYTES[0] == 0x81 {
                    buffer[0] = camera_id.to_address_byte();
                }

                Ok(LEN)
            }

            fn response_type(&self) -> Option<$crate::command::response::ResponseType> {
                $response
            }

        }
    };
}

/// Macro for creating compile-time VISCA command arrays.
/// Automatically adds 0xFF terminator.
///
/// This is an internal utility macro used by the command implementation.
macro_rules! visca_bytes {
    // Fixed bytes only
    ($($byte:expr),+ $(,)?) => {
        {
            const BYTES: &[u8] = &[$($byte),+, 0xFF];
            BYTES
        }
    };
}

/// Macro for creating VISCA command prefixes (without terminator).
///
/// This is an internal utility macro used by the command implementation.
macro_rules! visca_prefix {
    ($($byte:expr),+ $(,)?) => {
        &[$($byte),+]
    };
}

// Make internal macros available within the crate
pub(crate) use visca_bool_command;
pub(crate) use visca_builder;
pub(crate) use visca_bytes;
pub(crate) use visca_command;
pub(crate) use visca_const_command;
pub(crate) use visca_param_command;
pub(crate) use visca_prefix;
