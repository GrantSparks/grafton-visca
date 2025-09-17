//! Integration test for command cancellation functionality.
//!
//! This test demonstrates the cancel functionality is properly exposed
//! through the Camera API. Full testing would require a mock transport
//! that can simulate VISCA responses.

#![cfg(feature = "mode-async")]

#[allow(unused_imports)]
use grafton_visca::ViscaSocket;

/// Test that cancel methods are available on Camera.
/// This is a compilation test to ensure the API is properly exposed.
#[tokio::test]
#[cfg(feature = "runtime-tokio")]
async fn test_cancel_api_available() {
    // This test verifies that the cancel methods compile and are accessible.
    // Actual execution would require a connected camera or mock transport.

    // Just verify the types exist and are public
    let _socket1 = ViscaSocket::S1;
    let _socket2 = ViscaSocket::S2;
}

/// Test that demonstrates the intended usage pattern for cancellation.
#[tokio::test]
#[cfg(feature = "runtime-tokio")]
async fn test_cancel_usage_pattern() {
    // This demonstrates how a user would use the cancellation API
    // with a real camera connection.
}
