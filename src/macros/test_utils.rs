//! Test utility macros for VISCA command testing.
//!
//! This module contains macros used internally for testing VISCA command encoding.

/// Internal macro for test generation
///
/// This macro simplifies the creation of test cases for VISCA command encoding.
/// It automatically sets up the command, encodes it with a camera ID, and verifies
/// the resulting byte sequence.
///
/// # Example
/// ```ignore
/// #[cfg(test)]
/// mod tests {
///     use crate::macros::test_utils::visca_test;
///
///     visca_test!(Power, test_power_on,
///         Power::new(true),
///         &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
///     );
/// }
/// ```
macro_rules! visca_test {
    ($name:ident, $test_name:ident, $cmd:expr, $expected:expr) => {
        #[test]
        fn $test_name() {
            use $crate::command::encode::WireEncode;
            use $crate::CameraId;

            let cmd = $cmd;
            let mut buffer = vec![0u8; 32];
            let len = cmd
                .write_into(CameraId::CAMERA_1, &mut buffer)
                .expect("encode failed");
            assert_eq!(&buffer[..len], $expected);
        }
    };
}

pub(crate) use visca_test;
