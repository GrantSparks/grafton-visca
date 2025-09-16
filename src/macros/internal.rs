//! Internal macros for VISCA command implementation.
//!
//! This module contains macros used internally by the library for implementing
//! VISCA commands. These macros are not part of the public API and may change
//! without notice.
//!
//! ## New Consolidated Macros (Issue #397)
//!
//! - `visca_cmd!` - Unified macro for commands expecting ACK/Completion responses
//! - `visca_inquiry!` - Unified macro for inquiry commands expecting data responses
//!
//! ## Legacy Macros (Maintained for Backwards Compatibility)
//!
//! - Command generators: `visca_command!`, `visca_bool_command!`, `visca_builder!`,
//!   `visca_param_command!`, `visca_const_command!`
//! - Const utilities: `visca_bytes!`, `visca_prefix!`
//!
//! The legacy macros will be gradually phased out in favor of the new consolidated macros.

// str_to_command_category and str_bytes functions removed - now using typed parameters

/// Create a simple VISCA command enum with byte sequences.
///
/// This macro generates a complete implementation of the `Command` trait
/// for simple commands that don't require parameters.
///
/// The body of each variant should build and return a CommandBuilder.
/// The macro will handle camera ID and termination.
macro_rules! visca_command {
    (
        $(#[$meta:meta])*
        category = $category:expr,
        max_size = $max_size:literal,
        enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident $( ($($param:ident : $ptype:ty),*) )? => $body:expr
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

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = $max_size;
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                use $crate::command::bytes::ConstCommandBuilder;

                // Build the command directly without Vec allocation
                match self {
                    $(
                        Self::$variant$( ($($param),*) )? => {
                            // Execute the body which should return any ConstCommandBuilder<N, _>
                            let builder: Result<_, $crate::Error> = { $body };
                            let builder = builder?;

                            // Apply camera ID and terminate
                            let terminated = builder.with_camera_id(camera_id).terminate();
                            terminated.build_into(buffer)
                        },
                    )+
                }
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
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

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = $prefix_const.len() + 2; // prefix + state + 0xFF
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::Quick;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                // Use type-state pattern for compile-time safety
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append($prefix_const)
                    .push(if self.enabled { $on } else { $off })
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
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

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = [$($prefix),+].len() + 2; // prefix + state + 0xFF
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::Quick;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                // Use type-state pattern for compile-time safety
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(&[$($prefix),+])
                    .push(if self.enabled { $on } else { $off })
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
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
                #[doc = concat!("The ", stringify!($field), " parameter.")]
                pub $field: $ftype,
            )+
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = $size;
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                // Use ownership-based type-state pattern
                // The body must return the builder after chaining operations
                let $builder = $crate::command::bytes::ConstCommandBuilder::<$size>::new();

                let $builder = {
                    $(let $param = &self.$field;)+
                    $body
                };

                let terminated = $builder.with_camera_id(camera_id).terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
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

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = $prefix_const.len() + 2; // prefix + param + 0xFF
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                // Use type-state pattern for compile-time terminator safety
                let $field = &self.$field;
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append($prefix_const)
                    .push($param_expr)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
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

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = [$($prefix),+].len() + 2; // prefix + param + 0xFF
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                // Use type-state pattern for compile-time terminator safety
                let $field = &self.$field;
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(&[$($prefix),+])
                    .push($param_expr)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
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

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = $prefix_const.len() + 2; // prefix + param + 0xFF
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                // Use type-state pattern for compile-time terminator safety
                let $field = &self.$field;
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append($prefix_const)
                    .push($param_expr)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
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
    // Original form without optional parameters (backward compatibility)
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

    // Extended form with optional parameters (backward compatibility)
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

            /// Command bytes as a const array.
            const BYTES: &'static [u8] = &[$($byte),+];
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            // Calculate exact MAX_SIZE based on whether bytes include terminator
            const MAX_SIZE: usize = {
                const BYTES: &[u8] = &[$($byte),+];
                if BYTES.len() > 0 && BYTES[BYTES.len() - 1] == $crate::command::bytes::VISCA_TERMINATOR {
                    BYTES.len() // Already includes terminator
                } else {
                    BYTES.len() + 1 // Need to add terminator
                }
            };
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::error::Error> {
                const BYTES: &[u8] = $name::BYTES;

                // Use type-state pattern - handle pre-terminated commands
                if BYTES.last() == Some(&$crate::command::bytes::VISCA_TERMINATOR) {
                    // Command already has terminator, just substitute camera ID
                    let mut builder = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new();
                    builder.append_mut(BYTES);
                    builder.with_camera_id_mut(camera_id);
                    builder.terminate().build_into(buffer)
                } else {
                    // Use type-state pattern for proper termination
                    let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                        .append(BYTES)
                        .with_camera_id(camera_id)
                        .terminate();
                    terminated.build_into(buffer)
                }
            }

            fn response_kind(&self) -> Option<$crate::command::response::ResponseKind> {
                $response
            }

        }
    };

    // New form: bytes already include 0xFF terminator - with response
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        bytes_terminated = [$($byte:expr),+ $(,)?];
        timeout = $category:ident;
        address = $address:expr,
        response = $response:expr,
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name;

        impl $name {
            /// Create a new instance of this command.
            pub const fn new() -> Self {
                Self
            }

            /// Command bytes as a const array (already terminated).
            const BYTES: &'static [u8] = &[$($byte),+];
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = { [$($byte),+].len() }; // Exact size - already includes terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::error::Error> {
                const BYTES: &[u8] = $name::BYTES;

                // Use properly sized builder for pre-terminated commands
                let mut builder = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new();
                builder.append_mut(BYTES);
                builder.with_camera_id_mut(camera_id);
                builder.terminate().build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::response::ResponseKind> {
                Some($response)
            }
        }
    };

    // New form: bytes already include 0xFF terminator - without response
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        bytes_terminated = [$($byte:expr),+ $(,)?];
        timeout = $category:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name;

        impl $name {
            /// Create a new instance of this command.
            pub const fn new() -> Self {
                Self
            }

            /// Command bytes as a const array (already terminated).
            const BYTES: &'static [u8] = &[$($byte),+];
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = { [$($byte),+].len() }; // Exact size - already includes terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::error::Error> {
                const BYTES: &[u8] = $name::BYTES;

                // Use properly sized builder for pre-terminated commands
                let mut builder = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new();
                builder.append_mut(BYTES);
                builder.with_camera_id_mut(camera_id);
                builder.terminate().build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::response::ResponseKind> {
                None
            }
        }
    };

    // New form: prefix without terminator (will add terminator) - with response
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        prefix = [$($byte:expr),+ $(,)?];
        timeout = $category:ident;
        address = $address:expr,
        response = $response:expr,
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name;

        impl $name {
            /// Create a new instance of this command.
            pub const fn new() -> Self {
                Self
            }

            /// Command prefix as a const array (without terminator).
            const PREFIX: &'static [u8] = &[$($byte),+];
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = { [$($byte),+].len() + 1 }; // Prefix length + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::error::Error> {
                const PREFIX: &[u8] = $name::PREFIX;

                // Use properly sized builder and add termination
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(PREFIX)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::response::ResponseKind> {
                Some($response)
            }
        }
    };

    // New form: prefix without terminator (will add terminator) - without response
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        prefix = [$($byte:expr),+ $(,)?];
        timeout = $category:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name;

        impl $name {
            /// Create a new instance of this command.
            pub const fn new() -> Self {
                Self
            }

            /// Command prefix as a const array (without terminator).
            const PREFIX: &'static [u8] = &[$($byte),+];
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = { [$($byte),+].len() + 1 }; // Prefix length + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $crate::timeout::CommandCategory::$category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::error::Error> {
                const PREFIX: &[u8] = $name::PREFIX;

                // Use properly sized builder and add termination
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(PREFIX)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::response::ResponseKind> {
                None
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
            const BYTES: &[u8] = &[$($byte),+, $crate::command::bytes::VISCA_TERMINATOR];
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

/// Create a VISCA command that expects ACK/Completion response.
///
/// This is the new consolidated macro that replaces `visca_command!`, `visca_bool_command!`,
/// `visca_param_command!`, `visca_const_command!`, and `visca_builder!`.
///
/// # Basic Usage
///
/// For simple commands with fixed bytes:
/// ```ignore
/// visca_cmd! {
///     /// Power on the camera
///     pub struct PowerOn;
///     bytes = [0x01, 0x04, 0x00, 0x02];
///     category = CommandCategory::Quick;
/// }
/// ```
///
/// For commands with parameters:
/// ```ignore
/// visca_cmd! {
///     pub struct ImageFlip { mode: Flip };
///     prefix = [0x01, 0x06, 0x61];
///     param = match mode { Flip::On => 0x02, Flip::Off => 0x03 };
///     category = CommandCategory::Quick;
/// }
/// ```
macro_rules! visca_cmd {
    // Simple command with fixed bytes
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        bytes = [$($byte:expr),+ $(,)?];
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name;

        impl $name {
            /// Create a new instance of this command.
            pub const fn new() -> Self {
                Self
            }

            /// Command bytes as a const array.
            const BYTES: &'static [u8] = &[$($byte),+];
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = [$($byte),+].len() + 1; // bytes + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::Error> {
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(Self::BYTES)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                None
            }
        }
    };

    // Command with parameter
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident { $field:ident: $ftype:ty };
        prefix = [$($prefix:expr),+ $(,)?];
        param = $param_expr:expr;
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name {
            /// The parameter value.
            pub $field: $ftype,
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = [$($prefix),+].len() + 2; // prefix + param + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::Error> {
                let $field = &self.$field;
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(&[$($prefix),+])
                    .push($param_expr)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                None
            }
        }
    };

    // Command with parameter using constant reference
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident { $field:ident: $ftype:ty };
        prefix = $prefix_const:expr;
        param = $param_expr:expr;
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name {
            /// The parameter value.
            pub $field: $ftype,
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = $prefix_const.len() + 2; // prefix + param + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::Error> {
                let $field = &self.$field;
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append($prefix_const)
                    .push($param_expr)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                None
            }
        }
    };
}

/// Create a VISCA inquiry command that expects a data response.
///
/// This is the new consolidated macro for inquiry commands that return data.
///
/// # Usage
///
/// ```ignore
/// visca_inquiry! {
///     pub struct ZoomPosition;
///     prefix = [0x09, 0x04, 0x47];
///     returns = ResponseKind::ZoomPosition;
///     category = CommandCategory::Quick;
/// }
/// ```
macro_rules! visca_inquiry {
    // Simple inquiry with fixed prefix
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        prefix = [$($prefix:expr),+ $(,)?];
        returns = $response:expr;
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name;

        impl $name {
            /// Create a new instance of this inquiry.
            pub const fn new() -> Self {
                Self
            }

            /// Inquiry prefix bytes as a const array.
            const PREFIX: &'static [u8] = &[$($prefix),+];
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = [$($prefix),+].len() + 1; // prefix + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::Error> {
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(Self::PREFIX)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                Some($response)
            }
        }
    };

    // Inquiry with constant reference
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        prefix = $prefix_const:expr;
        returns = $response:expr;
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        $vis struct $name;

        impl $name {
            /// Create a new instance of this inquiry.
            pub const fn new() -> Self {
                Self
            }

            /// Inquiry prefix bytes as a const array.
            const PREFIX: &'static [u8] = $prefix_const;
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $crate::command::encode::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = $prefix_const.len() + 1; // prefix + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::Error> {
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .append(Self::PREFIX)
                    .with_camera_id(camera_id)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                Some($response)
            }
        }
    };
}

// Make internal macros available within the crate
pub(crate) use visca_bool_command;
pub(crate) use visca_builder;
pub(crate) use visca_bytes;
pub(crate) use visca_cmd;
pub(crate) use visca_command;
pub(crate) use visca_const_command;
pub(crate) use visca_param_command;
pub(crate) use visca_prefix;
