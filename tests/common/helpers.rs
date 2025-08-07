#![allow(missing_docs)]
//! Test helper functions to reduce unwrap usage and improve error messages.
//!
//! This module provides reusable test utilities that make tests more maintainable
//! and provide better failure messages, reducing the need for #[allow(...)] directives.
#![allow(dead_code)] // These utilities are for future test use

use std::fmt::Debug;

/// VISCA command terminator byte.
const VISCA_TERMINATOR: u8 = 0xFF;

// Note: ViscaProtocol creation helpers have been removed since concrete
// transport implementations (TCP/UDP) are now provided as examples
// rather than being part of the core library. Tests should either:
// 1. Use mock transports for unit testing
// 2. Include transport implementations from the examples directory
// 3. Implement their own test transports

/// Standard test speeds to avoid repetitive magic numbers.
///
/// Returns commonly used pan and tilt speeds for tests.
#[cfg(not(feature = "async"))]
pub fn test_speeds() -> (u8, u8) {
    (10, 10)
}

/// Test speeds with custom values.
#[cfg(not(feature = "async"))]
pub fn test_speeds_with(pan: u8, tilt: u8) -> (u8, u8) {
    // In the new API, speeds are just u8 values
    // Camera profiles handle validation
    (pan, tilt)
}

/// Assert that a Result is Ok and return the value with context.
///
/// Provides better error messages than unwrap() by including context about
/// what operation failed.
///
/// # Example
/// ```no_run
/// # use grafton_visca::tests::common::helpers::assert_ok;
/// let result = some_operation();
/// let value = assert_ok(result, "Expected operation to succeed");
/// ```
pub fn assert_ok<T, E: Debug>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(e) => panic!("{context}: {e:?}"),
    }
}

/// Assert that a Result is Err and return the error with context.
///
/// Useful for testing error conditions with better failure messages.
pub fn assert_err<T: Debug, E>(result: Result<T, E>, context: &str) -> E {
    match result {
        Ok(value) => panic!("{context}: Expected error but got Ok({value:?})"),
        Err(e) => e,
    }
}

/// Assert that two byte arrays are equal with hex formatting.
///
/// Provides better output for debugging protocol-level issues.
pub fn assert_bytes_eq(actual: &[u8], expected: &[u8], context: &str) {
    if actual != expected {
        panic!("{context}\nExpected: {expected:02X?}\nActual:   {actual:02X?}");
    }
}

/// Assert that a byte array starts with the given prefix.
pub fn assert_bytes_start_with(actual: &[u8], prefix: &[u8], context: &str) {
    if !actual.starts_with(prefix) {
        panic!("{context}\nExpected to start with: {prefix:02X?}\nActual: {actual:02X?}");
    }
}

/// Creates a valid VISCA ACK response for testing.
pub fn create_ack_response(socket: u8) -> Vec<u8> {
    vec![0x90, 0x40 | (socket & 0x01), VISCA_TERMINATOR]
}

/// Creates a valid VISCA completion response for testing.
pub fn create_completion_response(socket: u8) -> Vec<u8> {
    vec![0x90, 0x50 | (socket & 0x01), VISCA_TERMINATOR]
}

/// Creates a valid VISCA error response for testing.
pub fn create_error_response(error_code: u8) -> Vec<u8> {
    vec![0x90, 0x60, error_code, VISCA_TERMINATOR]
}

/// Creates a standard ACK + Completion sequence for a socket.
pub fn create_ack_completion_sequence(socket: u8) -> Vec<Vec<u8>> {
    vec![
        create_ack_response(socket),
        create_completion_response(socket),
    ]
}

#[cfg(all(test, not(feature = "async")))]
mod tests {
    use super::*;

    #[test]
    fn test_assert_ok_success() {
        let result: Result<i32, &str> = Ok(42);
        let value = assert_ok(result, "Should return 42");
        assert_eq!(value, 42);
    }

    #[test]
    #[should_panic(expected = "Operation failed: \"error message\"")]
    fn test_assert_ok_failure() {
        let result: Result<i32, &str> = Err("error message");
        assert_ok(result, "Operation failed");
    }

    #[test]
    fn test_assert_err_success() {
        let result: Result<i32, &str> = Err("expected error");
        let error = assert_err(result, "Should return error");
        assert_eq!(error, "expected error");
    }

    #[test]
    fn test_assert_bytes_eq_success() {
        let actual = vec![0x81, 0x01, 0x06, 0x04, VISCA_TERMINATOR];
        let expected = vec![0x81, 0x01, 0x06, 0x04, VISCA_TERMINATOR];
        assert_bytes_eq(&actual, &expected, "Bytes should match");
    }

    #[test]
    #[should_panic(
        expected = "Bytes mismatch\nExpected: [81, 01, 06, 04, FF]\nActual:   [81, 01, 06, 05, FF]"
    )]
    fn test_assert_bytes_eq_failure() {
        let actual = vec![0x81, 0x01, 0x06, 0x05, VISCA_TERMINATOR];
        let expected = vec![0x81, 0x01, 0x06, 0x04, VISCA_TERMINATOR];
        assert_bytes_eq(&actual, &expected, "Bytes mismatch");
    }

    #[test]
    fn test_create_responses() {
        assert_eq!(create_ack_response(0), vec![0x90, 0x40, VISCA_TERMINATOR]);
        assert_eq!(create_ack_response(1), vec![0x90, 0x41, VISCA_TERMINATOR]);
        assert_eq!(
            create_completion_response(0),
            vec![0x90, 0x50, VISCA_TERMINATOR]
        );
        assert_eq!(
            create_completion_response(1),
            vec![0x90, 0x51, VISCA_TERMINATOR]
        );
        assert_eq!(
            create_error_response(0x01),
            vec![0x90, 0x60, 0x01, VISCA_TERMINATOR]
        );
    }
}
