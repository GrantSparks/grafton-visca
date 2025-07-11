//! Macros for compile-time VISCA command creation.

/// Macro for creating compile-time VISCA command arrays.
/// Automatically adds 0xFF terminator.
///
/// # Example
/// ```
/// const POWER_ON: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x00, 0x02];
/// // Results in: [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
/// ```
#[macro_export]
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
/// # Example
/// ```
/// const ZOOM_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x07];
/// ```
#[macro_export]
macro_rules! visca_prefix {
    ($($byte:expr),+ $(,)?) => {
        &[$($byte),+]
    };
}
