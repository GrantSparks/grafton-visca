//! Internal macros for VISCA command implementation.
//!
//! This module contains macros used internally by the library for implementing
//! VISCA commands. These macros are not part of the public API and may change
//! without notice.
//!
//! ## Consolidated Macros
//!
//! - `visca_cmd!` - Unified macro for commands expecting ACK/Completion responses
//! - `visca_bytes!`, `visca_prefix!` - Const utilities for byte sequences

// str_to_command_category and str_bytes functions removed - now using typed parameters

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
/// This is the consolidated macro for commands expecting ACK/Completion responses.
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
#[allow(unused_macros)]
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
            const MAX_SIZE: usize = [$($byte),+].len() + 2; // camera_id + bytes + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::Error> {
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE + 1}>::new()
                    .push(camera_id.to_address_byte())
                    .append(Self::BYTES)
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
            const MAX_SIZE: usize = 1 + [$($prefix),+].len() + 1 + 1; // camera_id + prefix + param + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::Error> {
                let $field = &self.$field;
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .push(camera_id.to_address_byte())
                    .append(&[$($prefix),+])
                    .push($param_expr)
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
            const MAX_SIZE: usize = 1 + $prefix_const.len() + 2; // camera_id + prefix + param + terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(
                &self,
                camera_id: $crate::camera_id::CameraId,
                buffer: &mut [u8],
            ) -> Result<usize, $crate::Error> {
                let $field = &self.$field;
                let terminated = $crate::command::bytes::ConstCommandBuilder::<{Self::MAX_SIZE}>::new()
                    .push(camera_id.to_address_byte())
                    .append($prefix_const)
                    .push($param_expr)
                    .terminate();
                terminated.build_into(buffer)
            }

            fn response_kind(&self) -> Option<$crate::command::ResponseKind> {
                None
            }
        }
    };
}

// Make internal macros available within the crate
pub(crate) use visca_bytes;
pub(crate) use visca_prefix;
