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

                let bytes = match self {
                    $(
                        Self::$variant$( ($($param),*) )? => $variant($($($param),*)?)?,
                    )+
                };

                // Build command using CommandBuilder with type-state pattern
                // Use a conservative size for the builder
                let builder = $crate::command::const_encoding::CommandBuilder::<16>::new();

                // Append all bytes except potentially the terminator
                let has_terminator = bytes.last() == Some(&$crate::command::const_encoding::VISCA_TERMINATOR);
                let bytes_to_add = if has_terminator {
                    &bytes[..bytes.len() - 1]
                } else {
                    &bytes[..]
                };

                // Build command using type-state pattern
                let mut builder = builder;
                for byte in bytes_to_add {
                    builder = builder.push(*byte);
                }

                let terminated = builder.with_camera_id(camera_id).terminate();
                terminated.copy_to(buffer)
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
    // Form with constant reference for prefix
    (
        $(#[$meta:meta])*
        struct $name:ident {
            prefix: $prefix_const:expr,
            on: $on:expr,
            off: $off:expr,
        }
    ) => {
        visca_bool_command! {
            $(#[$meta])*
            struct $name {
                prefix: $prefix_const,
                on: $on,
                off: $off,
                address: 0x81,
                response: None,
            }
        }
    };

    // Form with constant reference and optional parameters
    (
        $(#[$meta:meta])*
        struct $name:ident {
            prefix: $prefix_const:expr,
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
            const MAX_SIZE: usize = $prefix_const.len() + 2; // prefix + state + 0xFF
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::Quick;

            fn encode_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                // Use type-state pattern for compile-time safety
                let terminated = $crate::command::const_encoding::CommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append($prefix_const)
                    .push(if self.enabled { $on } else { $off })
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.copy_to(buffer)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                $response
            }
        }
    };

    // Original form without optional parameters (for backwards compatibility)
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

    // Extended form with optional parameters (for backwards compatibility)
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
                // Use type-state pattern for compile-time safety
                let terminated = $crate::command::const_encoding::CommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(&[$($prefix),+])
                    .push(if self.enabled { $on } else { $off })
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.copy_to(buffer)
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
        builder<$size:literal> => |$builder:ident, $($param:ident),+| $body:block
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

                // Use ownership-based type-state pattern
                // The body must return the builder after chaining operations
                let $builder = $crate::command::const_encoding::CommandBuilder::<$size>::new();

                let $builder = {
                    $(let $param = &self.$field;)+
                    $body
                };

                let terminated = $builder.with_camera_id(camera_id).terminate();
                terminated.copy_to(buffer)
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

    // Form with constant reference for prefix
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
            prefix = $prefix_const;
            param_byte = $param_expr;
            timeout = $category;
            address = 0x81;
            response = None;
        }
    };

    // Extended form with constant reference and optional parameters
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $field:ident: $ftype:ty,
        }
        prefix = $prefix_const:expr;
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
                // Use type-state pattern for compile-time terminator safety
                let $field = &self.$field;
                let terminated = $crate::command::const_encoding::CommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append($prefix_const)
                    .push($param_expr)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.copy_to(buffer)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                $response
            }
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
                // Use type-state pattern for compile-time terminator safety
                let $field = &self.$field;
                let terminated = $crate::command::const_encoding::CommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(&[$($prefix),+])
                    .push($param_expr)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.copy_to(buffer)
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
                // Use type-state pattern for compile-time terminator safety
                let $field = &self.$field;
                let terminated = $crate::command::const_encoding::CommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append($prefix_const)
                    .push($param_expr)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.copy_to(buffer)
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

                // Use type-state pattern - handle pre-terminated commands
                if BYTES.last() == Some(&$crate::command::const_encoding::VISCA_TERMINATOR) {
                    // Command already has terminator, just substitute camera ID
                    let mut builder = $crate::command::const_encoding::CommandBuilder::<16>::new();
                    builder.append_mut(BYTES);
                    builder.with_camera_id_mut(camera_id);
                    builder.copy_to(buffer)
                } else {
                    // Use type-state pattern for proper termination
                    let terminated = $crate::command::const_encoding::CommandBuilder::<16>::new()
                        .append(BYTES)
                        .with_camera_id(camera_id)
                        .terminate();
                    terminated.copy_to(buffer)
                }
            }

            fn response_type(&self) -> Option<$crate::command::response::ResponseType> {
                $response
            }

        }
    };
}

/// Macro for creating compile-time VISCA command arrays.
/// Automatically adds VISCA_TERMINATOR (0xFF).
///
/// This is an internal utility macro used by the command implementation.
macro_rules! visca_bytes {
    // Fixed bytes only
    ($($byte:expr),+ $(,)?) => {
        {
            const BYTES: &[u8] = &[$($byte),+, $crate::command::const_encoding::VISCA_TERMINATOR];
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
