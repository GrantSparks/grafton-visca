//! Support macros for internal implementation.
//!
//! This module contains helper macros used internally by the library.
//! These macros are not part of the public API and may change without notice.

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

// Make these macros available within the crate
pub(crate) use visca_bytes;
pub(crate) use visca_prefix;
